use aes::Aes256;
use base64::Engine;
use ctr::cipher::{KeyIvInit, StreamCipher};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{cmp::Ordering, thread, time::Duration};

const GRAPHQL_URL: &str = "https://api.allanime.day/api";
const SITE_URL: &str = "https://allmanga.to";
const YOUTU_CHAN_URL: &str = "https://youtu-chan.com";
const ALLANIME_PLAYBACK_URL: &str = "https://allanime.day";
const ALLANIME_IMAGE_BASE_URL: &str = "https://allanime.day";
const SEARCH_RESULT_LIMIT: usize = 26;
const FIRST_SEARCH_PAGE: i64 = 1;
const DEFAULT_TRANSLATION_TYPE: &str = "sub";
const DEFAULT_COUNTRY_ORIGIN: &str = "JP";
const DEFAULT_SEARCH_SORT: &str = "Latest_Update";
const FIRST_EPISODE_NUMBER: f64 = 0.0;
const LAST_EPISODE_NUMBER: f64 = 9999.0;
const GRAPHQL_REQUEST_TIMEOUT_SECONDS: u64 = 20;
const EPISODE_SOURCE_REQUEST_TIMEOUT_SECONDS: u64 = 12;
const CLOCK_REQUEST_TIMEOUT_SECONDS: u64 = 8;
const MAX_CLOCK_SOURCE_REQUESTS_PER_TRANSLATION: usize = 3;
const MAX_CLOCK_SOURCE_REQUESTS_TOTAL: usize = 9;
const EPISODE_SOURCES_QUERY_HASH: &str =
    "d405d0edd690624b66baba3068e0edc3ac90f1597d898a1ec8db4e5c43c00fec";
const EPISODE_SOURCES_DECRYPTION_KEY: &str = "Xot36i3lK3:v1";
const CLOCK_JSON_URL: &str = "https://allanime.day/apivtwo/clock.json";
const CLOCK_DR_JSON_URL: &str = "https://allanime.day/apivtwo/clock/dr.json";
const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

const SHOW_FIELDS: &str = "_id name englishName nativeName thumbnail episodeCount score type status genres availableEpisodes season altNames countryOfOrigin";
const EPISODE_FIELDS: &str =
    "_id episodeIdNum notes thumbnails vidInforssub vidInforsdub vidInforsraw";

type Aes256Ctr = ctr::Ctr128BE<Aes256>;

#[derive(Clone)]
struct TranslationSourceContext {
    translation_label: &'static str,
    translation_type: &'static str,
    stream_info: Value,
}

struct TranslationSourceBatch {
    context: TranslationSourceContext,
    source_values: Vec<Value>,
}

#[derive(Clone)]
struct ClockSourceRequest {
    context: TranslationSourceContext,
    source_value: Value,
    clock_json_url: String,
}

struct ClockSourceResponse {
    request: ClockSourceRequest,
    clock_link_values: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct AnimeSearchResult {
    pub show_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub thumbnail_url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AnimeEpisodeResult {
    pub episode_number: String,
    pub title: String,
    pub subtitle: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AnimeStreamResult {
    pub stream_url: String,
    pub quality: Option<String>,
    pub subtitle: Option<String>,
    pub http_headers: Vec<(String, String)>,
}

pub fn search_anime(search_query: &str) -> Result<Vec<AnimeSearchResult>, String> {
    let query = build_search_query(search_query);
    let response_json = send_graphql_query(&query)?;
    let show_values = response_json
        .pointer("/data/shows/edges")
        .and_then(Value::as_array)
        .ok_or_else(|| "AllAnime returned no search results list.".to_string())?;

    Ok(show_values
        .iter()
        .filter_map(parse_anime_search_result)
        .collect())
}

pub fn fetch_episodes(show_id: &str) -> Result<Vec<AnimeEpisodeResult>, String> {
    let response_json =
        fetch_episode_info_range(show_id, FIRST_EPISODE_NUMBER, LAST_EPISODE_NUMBER)?;
    let mut episode_values = response_json
        .pointer("/data/episodeInfos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    episode_values.sort_by(compare_episode_values);

    Ok(episode_values
        .iter()
        .filter_map(parse_anime_episode_result)
        .collect())
}

pub fn fetch_streams(
    show_id: &str,
    episode_number: &str,
) -> Result<Vec<AnimeStreamResult>, String> {
    let target_episode_number = episode_number
        .parse::<f64>()
        .map_err(|_| format!("AllAnime episode number is invalid: {episode_number}"))?;
    let response_json =
        fetch_episode_info_range(show_id, target_episode_number, target_episode_number)?;
    let episode_values = response_json
        .pointer("/data/episodeInfos")
        .and_then(Value::as_array)
        .ok_or_else(|| "AllAnime returned no episode stream data.".to_string())?;
    let episode_value = episode_values
        .iter()
        .find(|episode_value| episode_number_key(*episode_value) == episode_number)
        .or_else(|| episode_values.first())
        .ok_or_else(|| "AllAnime returned no matching episode stream data.".to_string())?;
    let stream_results = build_source_stream_results(show_id, episode_number, episode_value)?;

    if stream_results.is_empty() {
        Err("AllAnime did not return playable source streams for this episode.".to_string())
    } else {
        Ok(stream_results)
    }
}

fn build_search_query(search_query: &str) -> String {
    let search_literal = graphql_string_literal(search_query);
    format!(
        "{{shows(search:{{sortBy:{DEFAULT_SEARCH_SORT},query:{search_literal}}},limit:{SEARCH_RESULT_LIMIT},page:{FIRST_SEARCH_PAGE},translationType:{DEFAULT_TRANSLATION_TYPE},countryOrigin:{DEFAULT_COUNTRY_ORIGIN}){{edges{{{SHOW_FIELDS}}}}}}}"
    )
}

fn fetch_episode_info_range(
    show_id: &str,
    episode_start: f64,
    episode_end: f64,
) -> Result<Value, String> {
    let show_id_literal = graphql_string_literal(show_id);
    let query = format!(
        "{{episodeInfos(showId:{show_id_literal},episodeNumStart:{episode_start},episodeNumEnd:{episode_end}){{{EPISODE_FIELDS}}}}}"
    );

    send_graphql_query(&query)
}

fn send_graphql_query(query: &str) -> Result<Value, String> {
    let response = ureq::post(GRAPHQL_URL)
        .set("Origin", SITE_URL)
        .set("Referer", "https://allmanga.to/")
        .set("User-Agent", BROWSER_USER_AGENT)
        .set("Accept", "application/json, text/plain, */*")
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(GRAPHQL_REQUEST_TIMEOUT_SECONDS))
        .send_json(json!({ "query": query }))
        .map_err(|error| format!("AllAnime request failed: {error}"))?;
    let response_json = response
        .into_json::<Value>()
        .map_err(|error| format!("AllAnime returned invalid JSON: {error}"))?;

    if let Some(error_message) = first_graphql_error_message(&response_json) {
        Err(format!("AllAnime error: {error_message}"))
    } else {
        Ok(response_json)
    }
}

fn first_graphql_error_message(response_json: &Value) -> Option<String> {
    response_json
        .get("errors")
        .and_then(Value::as_array)
        .and_then(|errors| errors.first())
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn parse_anime_search_result(show_value: &Value) -> Option<AnimeSearchResult> {
    let show_id = text_json_field(show_value, &["_id", "id"])?;
    let title = text_json_field(show_value, &["englishName", "name", "nativeName"])?;
    let subtitle = search_result_subtitle(show_value);
    let thumbnail_url = string_json_field(show_value, &["thumbnail"])
        .map(|thumbnail_path| anime_thumbnail_url(&thumbnail_path));

    Some(AnimeSearchResult {
        show_id,
        title,
        subtitle,
        thumbnail_url,
    })
}

fn search_result_subtitle(show_value: &Value) -> Option<String> {
    let mut subtitle_parts = Vec::new();

    if let Some(anime_type) = text_json_field(show_value, &["type"]) {
        subtitle_parts.push(anime_type);
    }
    if let Some(status) = text_json_field(show_value, &["status"]) {
        subtitle_parts.push(status);
    }
    if let Some(episode_count) = text_json_field(show_value, &["episodeCount"]) {
        subtitle_parts.push(format!("{episode_count} episode(s)"));
    }
    if subtitle_parts.is_empty() {
        text_json_field(show_value, &["season", "countryOfOrigin"])
    } else {
        Some(subtitle_parts.join(" / "))
    }
}

fn parse_anime_episode_result(episode_value: &Value) -> Option<AnimeEpisodeResult> {
    let episode_number = episode_number_key(episode_value);
    if episode_number.is_empty() {
        return None;
    }

    let (episode_title, episode_subtitle) =
        episode_title_and_subtitle(episode_value, &episode_number);
    Some(AnimeEpisodeResult {
        episode_number,
        title: episode_title,
        subtitle: episode_subtitle,
    })
}

fn episode_title_and_subtitle(
    episode_value: &Value,
    episode_number: &str,
) -> (String, Option<String>) {
    let notes = string_json_field(episode_value, &["notes"]).unwrap_or_default();
    let note_parts = notes
        .split("<note-split>")
        .map(clean_note_text)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let title = note_parts
        .first()
        .map(|note_title| format!("Episode {episode_number}: {note_title}"))
        .unwrap_or_else(|| format!("Episode {episode_number}"));
    let subtitle = note_parts.get(1).cloned();

    (title, subtitle)
}

fn build_source_stream_results(
    show_id: &str,
    episode_number: &str,
    episode_value: &Value,
) -> Result<Vec<AnimeStreamResult>, String> {
    let translation_contexts = available_translation_source_contexts(episode_value);
    let (source_batches, source_errors) =
        fetch_translation_source_batches(show_id, episode_number, translation_contexts);
    let mut stream_results = Vec::new();
    let mut stream_urls_seen = Vec::new();
    let mut clock_source_requests = Vec::new();

    for source_batch in &source_batches {
        collect_source_stream_candidates(
            &source_batch.source_values,
            &source_batch.context,
            &mut stream_results,
            &mut stream_urls_seen,
            &mut clock_source_requests,
        );
    }

    clock_source_requests.truncate(MAX_CLOCK_SOURCE_REQUESTS_TOTAL);
    for clock_source_response in fetch_clock_source_responses(clock_source_requests) {
        for clock_link_value in clock_source_response.clock_link_values {
            collect_clock_stream_result(
                &clock_source_response.request.source_value,
                &clock_link_value,
                clock_source_response.request.context.translation_label,
                &clock_source_response.request.context.stream_info,
                &mut stream_results,
                &mut stream_urls_seen,
            );
        }
    }

    if stream_results.is_empty() && !source_errors.is_empty() {
        Err(format!(
            "AllAnime source lookup failed: {}",
            source_errors.join("; ")
        ))
    } else {
        Ok(stream_results)
    }
}

fn available_translation_source_contexts(episode_value: &Value) -> Vec<TranslationSourceContext> {
    [
        ("Sub", "sub", "vidInforssub"),
        ("Dub", "dub", "vidInforsdub"),
        ("Raw", "raw", "vidInforsraw"),
    ]
    .into_iter()
    .filter_map(|(translation_label, translation_type, field_name)| {
        let Some(stream_info) = episode_value
            .get(field_name)
            .filter(|value| value.is_object())
        else {
            return None;
        };

        Some(TranslationSourceContext {
            translation_label,
            translation_type,
            stream_info: stream_info.clone(),
        })
    })
    .collect()
}

fn fetch_translation_source_batches(
    show_id: &str,
    episode_number: &str,
    translation_contexts: Vec<TranslationSourceContext>,
) -> (Vec<TranslationSourceBatch>, Vec<String>) {
    let source_handles = translation_contexts
        .into_iter()
        .map(|context| {
            let show_id = show_id.to_string();
            let episode_number = episode_number.to_string();
            let translation_label = context.translation_label;
            let source_handle = thread::spawn(move || {
                fetch_episode_source_values(&show_id, &episode_number, context.translation_type)
                    .map(|source_values| TranslationSourceBatch {
                        context,
                        source_values,
                    })
            });

            (translation_label, source_handle)
        })
        .collect::<Vec<_>>();

    let mut source_batches = Vec::new();
    let mut source_errors = Vec::new();

    for (translation_label, source_handle) in source_handles {
        match source_handle.join() {
            Ok(Ok(source_batch)) => source_batches.push(source_batch),
            Ok(Err(error_message)) => source_errors.push(error_message),
            Err(_) => source_errors.push(format!(
                "AllAnime {translation_label} source lookup failed unexpectedly."
            )),
        }
    }

    (source_batches, source_errors)
}

fn fetch_episode_source_values(
    show_id: &str,
    episode_number: &str,
    translation_type: &str,
) -> Result<Vec<Value>, String> {
    let response_json = send_episode_sources_query(show_id, episode_number, translation_type)?;
    let encrypted_sources = response_json
        .pointer("/data/tobeparsed")
        .and_then(Value::as_str)
        .ok_or_else(|| "AllAnime returned no encrypted source payload.".to_string())?;
    let decrypted_sources = decrypt_episode_sources(encrypted_sources)?;
    let sources_json = serde_json::from_str::<Value>(&decrypted_sources)
        .map_err(|error| format!("AllAnime returned invalid decrypted source JSON: {error}"))?;
    let mut source_values = sources_json
        .pointer("/episode/sourceUrls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    source_values.sort_by(compare_source_priority_descending);
    Ok(source_values)
}

fn send_episode_sources_query(
    show_id: &str,
    episode_number: &str,
    translation_type: &str,
) -> Result<Value, String> {
    let variables = json!({
        "showId": show_id,
        "translationType": translation_type,
        "episodeString": episode_number,
    })
    .to_string();
    let extensions = json!({
        "persistedQuery": {
            "version": 1,
            "sha256Hash": EPISODE_SOURCES_QUERY_HASH,
        }
    })
    .to_string();
    let request_url = format!(
        "{GRAPHQL_URL}?variables={}&extensions={}",
        percent_encode_query_component(&variables),
        percent_encode_query_component(&extensions),
    );
    let response = ureq::get(&request_url)
        .set("Origin", YOUTU_CHAN_URL)
        .set("Referer", YOUTU_CHAN_URL)
        .set("User-Agent", BROWSER_USER_AGENT)
        .set("Accept", "application/json, text/plain, */*")
        .timeout(Duration::from_secs(EPISODE_SOURCE_REQUEST_TIMEOUT_SECONDS))
        .call()
        .map_err(|error| format!("AllAnime source request failed: {error}"))?;
    let response_json = response
        .into_json::<Value>()
        .map_err(|error| format!("AllAnime returned invalid source JSON: {error}"))?;

    if let Some(error_message) = first_graphql_error_message(&response_json) {
        Err(format!("AllAnime source error: {error_message}"))
    } else {
        Ok(response_json)
    }
}

fn decrypt_episode_sources(encrypted_sources: &str) -> Result<String, String> {
    let encrypted_bytes = base64::engine::general_purpose::STANDARD
        .decode(encrypted_sources)
        .map_err(|error| format!("AllAnime source payload was not valid base64: {error}"))?;

    if encrypted_bytes.len() <= 29 {
        return Err("AllAnime source payload was too short to decrypt.".to_string());
    }

    let mut cipher_iv = [0u8; 16];
    cipher_iv[..12].copy_from_slice(&encrypted_bytes[1..13]);
    cipher_iv[15] = 2;

    let cipher_key = Sha256::digest(EPISODE_SOURCES_DECRYPTION_KEY.as_bytes());
    let mut decrypted_bytes = encrypted_bytes[13..encrypted_bytes.len() - 16].to_vec();
    let mut cipher = Aes256Ctr::new(&cipher_key, &cipher_iv.into());
    cipher.apply_keystream(&mut decrypted_bytes);

    String::from_utf8(decrypted_bytes)
        .map_err(|error| format!("AllAnime source payload was not valid UTF-8: {error}"))
}

fn collect_source_stream_candidates(
    source_values: &[Value],
    context: &TranslationSourceContext,
    stream_results: &mut Vec<AnimeStreamResult>,
    stream_urls_seen: &mut Vec<String>,
    clock_source_requests: &mut Vec<ClockSourceRequest>,
) {
    let mut translation_clock_request_count = 0;

    for source_value in source_values {
        let Some(raw_source_url) = string_json_field(source_value, &["sourceUrl", "url"]) else {
            continue;
        };
        let decoded_source_url = decode_allanime_source_url(&raw_source_url);

        if let Some(clock_json_url) = clock_json_url(&decoded_source_url) {
            if translation_clock_request_count < MAX_CLOCK_SOURCE_REQUESTS_PER_TRANSLATION {
                translation_clock_request_count += 1;
                clock_source_requests.push(ClockSourceRequest {
                    context: context.clone(),
                    source_value: source_value.clone(),
                    clock_json_url,
                });
            }
            continue;
        }

        if !should_use_direct_source_url(source_value, &decoded_source_url) {
            continue;
        }

        collect_direct_stream_result(
            source_value,
            &decoded_source_url,
            context.translation_label,
            &context.stream_info,
            stream_results,
            stream_urls_seen,
        );
    }
}

fn fetch_clock_source_responses(
    clock_source_requests: Vec<ClockSourceRequest>,
) -> Vec<ClockSourceResponse> {
    let clock_handles = clock_source_requests
        .into_iter()
        .map(|request| {
            let clock_json_url = request.clock_json_url.clone();
            thread::spawn(move || {
                fetch_clock_link_values(&clock_json_url)
                    .ok()
                    .map(|clock_link_values| ClockSourceResponse {
                        request,
                        clock_link_values,
                    })
            })
        })
        .collect::<Vec<_>>();

    clock_handles
        .into_iter()
        .filter_map(|clock_handle| clock_handle.join().ok().flatten())
        .collect()
}

fn collect_clock_stream_result(
    source_value: &Value,
    clock_link_value: &Value,
    translation_label: &str,
    stream_info: &Value,
    stream_results: &mut Vec<AnimeStreamResult>,
    stream_urls_seen: &mut Vec<String>,
) {
    let Some(stream_url) = source_url_from_clock_link(clock_link_value) else {
        return;
    };
    if stream_urls_seen
        .iter()
        .any(|seen_url| seen_url == &stream_url)
    {
        return;
    }

    let source_label = stream_source_label(
        source_value,
        clock_link_value,
        translation_label,
        stream_info,
    );
    stream_urls_seen.push(stream_url.clone());
    stream_results.push(AnimeStreamResult {
        stream_url,
        quality: Some(source_label.clone()),
        subtitle: Some(format!("AllAnime / {source_label}")),
        http_headers: playback_http_headers(),
    });
}

fn collect_direct_stream_result(
    source_value: &Value,
    stream_url: &str,
    translation_label: &str,
    stream_info: &Value,
    stream_results: &mut Vec<AnimeStreamResult>,
    stream_urls_seen: &mut Vec<String>,
) {
    if stream_urls_seen
        .iter()
        .any(|seen_url| seen_url == stream_url)
    {
        return;
    }

    let source_label =
        stream_source_label(source_value, &Value::Null, translation_label, stream_info);
    stream_urls_seen.push(stream_url.to_string());
    stream_results.push(AnimeStreamResult {
        stream_url: stream_url.to_string(),
        quality: Some(source_label.clone()),
        subtitle: Some(format!("AllAnime / {source_label}")),
        http_headers: playback_http_headers(),
    });
}

fn fetch_clock_link_values(clock_json_url: &str) -> Result<Vec<Value>, String> {
    let response = ureq::get(clock_json_url)
        .set("Referer", ALLANIME_PLAYBACK_URL)
        .set("User-Agent", BROWSER_USER_AGENT)
        .set("Accept", "application/json, text/plain, */*")
        .timeout(Duration::from_secs(CLOCK_REQUEST_TIMEOUT_SECONDS))
        .call()
        .map_err(|error| format!("AllAnime clock request failed: {error}"))?;
    let response_json = response
        .into_json::<Value>()
        .map_err(|error| format!("AllAnime clock returned invalid JSON: {error}"))?;

    Ok(response_json
        .get("links")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

fn source_url_from_clock_link(clock_link_value: &Value) -> Option<String> {
    source_url_json_field(clock_link_value, &["link", "src", "file"]).filter(|stream_url| {
        clock_link_value
            .get("mp4")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || clock_link_value
                .get("hls")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            || clock_link_value
                .get("dash")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            || is_direct_media_url(stream_url)
    })
}

fn clock_json_url(decoded_source_url: &str) -> Option<String> {
    let query_start = decoded_source_url.find('?')?;
    let source_path = &decoded_source_url[..query_start];
    let query_string = &decoded_source_url[query_start..];

    if source_path.ends_with("/apivtwo/clock")
        || source_path.ends_with("/apivtwo/clock.json")
        || source_path.ends_with("/apivtwo/clock/")
    {
        return Some(format!("{CLOCK_JSON_URL}{query_string}"));
    }
    if source_path.ends_with("/apivtwo/clock/dr")
        || source_path.ends_with("/apivtwo/clock/dr.json")
        || source_path.ends_with("/apivtwo/clock/dr/")
    {
        return Some(format!("{CLOCK_DR_JSON_URL}{query_string}"));
    }

    None
}

fn should_use_direct_source_url(source_value: &Value, stream_url: &str) -> bool {
    if !is_http_url(stream_url) {
        return false;
    }

    let source_type = text_json_field(source_value, &["type"])
        .unwrap_or_default()
        .to_ascii_lowercase();
    let fallback_type = text_json_field(source_value, &["fallBack", "fallback"])
        .unwrap_or_default()
        .to_ascii_lowercase();

    source_type == "player"
        || fallback_type == "mp4"
        || fallback_type == "hls"
        || is_direct_media_url(stream_url)
}

fn stream_source_label(
    source_value: &Value,
    clock_link_value: &Value,
    translation_label: &str,
    stream_info: &Value,
) -> String {
    let mut label_parts = Vec::new();

    if let Some(source_name) = text_json_field(source_value, &["sourceName", "name"]) {
        label_parts.push(source_name);
    }
    if let Some(clock_quality) = text_json_field(clock_link_value, &["resolutionStr", "quality"]) {
        label_parts.push(clock_quality);
    }
    if let Some(stream_quality) = stream_quality_label(translation_label, stream_info) {
        label_parts.push(stream_quality);
    }

    if label_parts.is_empty() {
        translation_label.to_string()
    } else {
        deduplicate_label_parts(label_parts).join(" / ")
    }
}

fn deduplicate_label_parts(label_parts: Vec<String>) -> Vec<String> {
    let mut unique_parts = Vec::new();

    for label_part in label_parts {
        if !unique_parts
            .iter()
            .any(|unique_part: &String| unique_part.eq_ignore_ascii_case(&label_part))
        {
            unique_parts.push(label_part);
        }
    }

    unique_parts
}

fn stream_quality_label(translation_label: &str, stream_info: &Value) -> Option<String> {
    let mut quality_parts = vec![translation_label.to_string()];

    if let Some(resolution) = text_json_field(stream_info, &["vidResolution"]) {
        let resolution_label = if resolution.ends_with('p') {
            resolution
        } else {
            format!("{resolution}p")
        };
        quality_parts.push(resolution_label);
    }
    if let Some(size_bytes) = stream_info.get("vidSize").and_then(Value::as_f64) {
        let size_mb = (size_bytes / 1_048_576.0 * 10.0).round() / 10.0;
        quality_parts.push(format!("{size_mb:.1} MB"));
    }

    (!quality_parts.is_empty()).then(|| quality_parts.join(" / "))
}

fn playback_http_headers() -> Vec<(String, String)> {
    vec![
        (
            "Referer".to_string(),
            format!("{}/", ALLANIME_PLAYBACK_URL.trim_end_matches('/')),
        ),
        ("User-Agent".to_string(), BROWSER_USER_AGENT.to_string()),
    ]
}

fn compare_source_priority_descending(left: &Value, right: &Value) -> Ordering {
    let left_priority = left
        .get("priority")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let right_priority = right
        .get("priority")
        .and_then(Value::as_f64)
        .unwrap_or_default();

    right_priority
        .partial_cmp(&left_priority)
        .unwrap_or(Ordering::Equal)
}

fn compare_episode_values(left: &Value, right: &Value) -> Ordering {
    let left_episode_number = left
        .get("episodeIdNum")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let right_episode_number = right
        .get("episodeIdNum")
        .and_then(Value::as_f64)
        .unwrap_or_default();

    left_episode_number
        .partial_cmp(&right_episode_number)
        .unwrap_or(Ordering::Equal)
}

fn episode_number_key(episode_value: &Value) -> String {
    episode_value
        .get("episodeIdNum")
        .and_then(Value::as_f64)
        .map(format_episode_number)
        .unwrap_or_default()
}

fn format_episode_number(episode_number: f64) -> String {
    if (episode_number.fract()).abs() < f64::EPSILON {
        format!("{}", episode_number as i64)
    } else {
        let formatted_number = format!("{episode_number:.3}");
        formatted_number
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn graphql_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn decode_allanime_source_url(raw_source_url: &str) -> String {
    if let Some(encoded_url) = raw_source_url.strip_prefix("--") {
        return decode_xor_hex_string(encoded_url, 56)
            .unwrap_or_else(|| raw_source_url.to_string());
    }
    if let Some(encoded_url) = raw_source_url.strip_prefix("ap/") {
        return decode_hex_string(encoded_url).unwrap_or_else(|| raw_source_url.to_string());
    }

    raw_source_url.to_string()
}

fn decode_xor_hex_string(encoded_value: &str, xor_key: u8) -> Option<String> {
    let decoded_bytes = decode_hex_bytes(encoded_value)?
        .into_iter()
        .map(|byte| byte ^ xor_key)
        .collect::<Vec<_>>();
    String::from_utf8(decoded_bytes).ok()
}

fn decode_hex_string(encoded_value: &str) -> Option<String> {
    String::from_utf8(decode_hex_bytes(encoded_value)?).ok()
}

fn decode_hex_bytes(encoded_value: &str) -> Option<Vec<u8>> {
    if encoded_value.len() % 2 != 0 {
        return None;
    }

    (0..encoded_value.len())
        .step_by(2)
        .map(|byte_index| u8::from_str_radix(&encoded_value[byte_index..byte_index + 2], 16).ok())
        .collect()
}

fn anime_thumbnail_url(path: &str) -> String {
    let trimmed_path = path.trim();

    if trimmed_path.starts_with("https://") || trimmed_path.starts_with("http://") {
        trimmed_path.to_string()
    } else if trimmed_path.starts_with("//") {
        format!("https:{trimmed_path}")
    } else {
        format!(
            "{}/{}",
            ALLANIME_IMAGE_BASE_URL.trim_end_matches('/'),
            trimmed_path.trim_start_matches('/')
        )
    }
}

fn clean_note_text(value: &str) -> String {
    collapse_whitespace(
        &value
            .replace("&nbsp;", " ")
            .replace("<br>", " ")
            .replace("<br/>", " "),
    )
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_json_field(value: &Value, candidate_keys: &[&str]) -> Option<String> {
    candidate_keys.iter().find_map(|key| {
        value.get(*key).and_then(|field_value| match field_value {
            Value::String(text) => {
                let trimmed_text = text.trim();
                (!trimmed_text.is_empty()).then(|| trimmed_text.to_string())
            }
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(boolean) => Some(boolean.to_string()),
            _ => None,
        })
    })
}

fn string_json_field(value: &Value, candidate_keys: &[&str]) -> Option<String> {
    candidate_keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|field_value| !field_value.is_empty())
            .map(ToString::to_string)
    })
}

fn source_url_json_field(value: &Value, candidate_keys: &[&str]) -> Option<String> {
    string_json_field(value, candidate_keys).filter(|url| is_http_url(url))
}

fn is_http_url(url: &str) -> bool {
    let lowercase_url = url.to_ascii_lowercase();
    lowercase_url.starts_with("https://") || lowercase_url.starts_with("http://")
}

fn is_direct_media_url(url: &str) -> bool {
    let lowercase_url = url.to_ascii_lowercase();
    lowercase_url.contains(".m3u8")
        || lowercase_url.contains(".mp4")
        || lowercase_url.contains("/download.aspx")
        || lowercase_url.contains("fast4speed")
        || lowercase_url.contains("videoplayback")
}

fn percent_encode_query_component(value: &str) -> String {
    let mut encoded_value = String::new();

    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.' | b'~') {
            encoded_value.push(*byte as char);
        } else {
            encoded_value.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded_value
}
