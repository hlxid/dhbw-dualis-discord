use std::{fs::File, io::BufReader, path::Path};

use dotenv::dotenv;
use regex::Regex;
use reqwest::blocking::{Client, ClientBuilder};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};

const BASE_URL: &str = "https://dualis.dhbw.de";
const FILE_PATH: &str = "./dualis_results.json";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv()?;

    let client = ClientBuilder::new().cookie_store(true).build()?;
    let auth_arguments = login(&client)?;
    let result_html = fetch_results(&client, &auth_arguments)?;
    let results = parse_results(&result_html);

    for result in results.iter() {
        println!(
            "id: {}, name: {}, scored: {}",
            result.course_id, result.course_name, result.scored
        );
    }

    let old_results = load_results();
    if let Some(old_results) = old_results {
        let changes = diff_results(&old_results, &results);
        for change in changes {
            handle_newly_scored_course(change)
        }
    } else {
        println!("No saved results found. Not looking for changes.");
    }

    save_results(&results)?;

    Ok(())
}

fn login(client: &Client) -> Result<String, Box<dyn std::error::Error>> {
    println!("Logging in...");
    let url = format!("{}/scripts/mgrqispi.dll", BASE_URL);

    let username = std::env::var("DUALIS_EMAIL")?;
    let password = std::env::var("DUALIS_PASSWORD")?;
    let form_data: &[(&str, &str)] = &[
        ("usrname", &username),
        ("pass", &password),
        ("APPNAME", "CampusNet"),
        ("PRGNAME", "LOGINCHECK"),
        (
            "ARGUMENTS",
            "clino,usrname,pass,menuno,menu_type,browser,platform",
        ),
        ("clino", "000000000000001"),
        ("menuno", "000324"),
        ("menu_type", "classic"),
        ("browser", ""),
        ("platform", ""),
    ];

    let response = client.post(url).form(form_data).send()?;

    // Response code should always be 200. If the response body is too large,
    // it usually means that the login failed because a html page with a error is returned.
    let status = response.status();
    let refresh_header = response
        .headers()
        .get("REFRESH")
        .ok_or("No refresh header found")
        .cloned();
    let content = response.text()?;

    if !status.is_success() || content.len() > 500 {
        return Err(format!(
            "Login failed. Please check your credentials. Status code: {}",
            status
        )
        .into());
    }

    println!("Login successful!");

    let refresh_header = refresh_header?;
    let refresh_header = refresh_header.to_str()?;

    // TODO: unuglify this constant substring
    Ok(refresh_header[84..].to_string())
}

fn fetch_results(
    client: &Client,
    auth_arguments: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    println!("Fetching results...");
    let url = format!(
        "{}/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=STUDENT_RESULT&ARGUMENTS={}",
        BASE_URL, auth_arguments
    );

    let response = client.get(url).send()?;
    let status = response.status();
    let content = response.text()?;

    if !status.is_success() || content.len() < 500 {
        return Err("Failed to fetch results.".into());
    }

    println!("Successfully fetched results!");

    Ok(content)
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
struct CourseResult {
    course_id: String,
    course_name: String,
    scored: bool,
}

impl CourseResult {
    fn new(course_id: String, course_name: String, scored: bool) -> Self {
        Self {
            course_id,
            course_name,
            scored,
        }
    }
}

fn parse_results(result_html: &str) -> Vec<CourseResult> {
    println!("Parsing results...");
    let document = Html::parse_document(result_html);
    let mut results = Vec::new();

    let course_name_replace_regex = Regex::new("<!--.+-->").unwrap();
    let table_rows_selector = Selector::parse("tbody tr").unwrap();
    let img_selector = Selector::parse("img").unwrap();

    let table_rows = document.select(&table_rows_selector);
    for row in table_rows {
        // Filter useless rows
        let row_classes = row.value().attr("class").unwrap_or_default();
        if row_classes.contains("subhead") || row_classes.contains("level00") {
            continue;
        }

        let cell_selector = Selector::parse("td").unwrap();
        let cells: Vec<ElementRef> = row.select(&cell_selector).collect();

        if cells.len() < 6 {
            continue;
        }

        if cells.iter().any(|cell| {
            !cell
                .value()
                .attr("class")
                .unwrap_or_default()
                .contains("tbdata")
        }) {
            continue;
        }

        // Initial parsing:
        let course_id: String = cells[0].text().collect();
        let course_name: String = cells[1].text().map(|text_part| text_part.trim()).collect();

        let title = cells[5]
            .select(&img_selector)
            .next()
            .map(|img| img.value().attr("title").unwrap_or("offen"));
        let scored = title.unwrap_or("offen").to_lowercase() != "offen";

        // Value fixing:
        // Dualis is so bad that they use xml/html comments inside a script tag LMAO
        // Replace line endings so everything is a single line for the regex.
        let course_name = course_name.replace('\n', "");
        let course_name = course_name_replace_regex
            .replace_all(&course_name, "")
            .to_string();

        let course_result = CourseResult::new(course_id, course_name, scored);
        results.push(course_result);
    }

    println!("Successfully parsed {} results!", results.len());
    results
}

fn load_results() -> Option<Vec<CourseResult>> {
    let path = Path::new(FILE_PATH);
    if !path.exists() {
        return None;
    }

    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);
    let results: Vec<CourseResult> = serde_json::from_reader(reader).unwrap();

    println!("Successfully loaded {} results!", results.len());
    Some(results)
}

fn save_results(results: &[CourseResult]) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string(&results)?;
    std::fs::write(FILE_PATH, json)?;
    println!("Successfully saved results to {FILE_PATH}.");

    Ok(())
}

fn diff_results<'a>(old: &[CourseResult], new: &'a [CourseResult]) -> Vec<&'a CourseResult> {
    println!("Looking for newly scored courses...");
    let mut changed = vec![];

    for entry in new {
        let old_entry = old
            .iter()
            .find(|old_entry| old_entry.course_id == entry.course_id);
        if old_entry.is_none() {
            continue;
        }

        if entry.scored && !old_entry.unwrap().scored {
            changed.push(entry);
        }
    }

    println!("Found {} newly scored courses!", changed.len());
    changed
}

fn handle_newly_scored_course(cr: &CourseResult) {
    println!("Newly scored: {}", cr.course_name);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_results() {
        let html = include_str!("../test_data/results.html");
        let results = parse_results(html);
        assert_eq!(results.len(), 23);

        assert_eq!(
            results,
            vec![
                CourseResult::new("T3INF1001".to_string(), "Mathematik I".to_string(), false),
                CourseResult::new("T3INF1002".to_string(), "Theoretische Informatik I".to_string(), true),
                CourseResult::new("T3INF1003".to_string(), "Theoretische Informatik II".to_string(), false),
                CourseResult::new("T3INF1004".to_string(), "Programmieren".to_string(), false),
                CourseResult::new("T3INF1005".to_string(), "Schlüsselqualifikationen".to_string(), false),
                CourseResult::new("T3INF1006".to_string(), "Technische Informatik I".to_string(), false),
                CourseResult::new("T3INF2001".to_string(), "Mathematik II".to_string(), false),
                CourseResult::new("T3INF2002".to_string(), "Theoretische Informatik III".to_string(), false),
                CourseResult::new("T3INF2003".to_string(), "Software Engineering I".to_string(), false),
                CourseResult::new("T3INF2004".to_string(), "Datenbanken".to_string(), false),
                CourseResult::new("T3INF2005".to_string(), "Technische Informatik II".to_string(), false),
                CourseResult::new("T3INF2006".to_string(), "Kommunikations- und Netztechnik".to_string(), false),
                CourseResult::new("T3INF3001".to_string(), "Software Engineering II".to_string(), false),
                CourseResult::new("T3INF3002".to_string(), "IT-Sicherheit".to_string(), false),
                CourseResult::new("T3_3101".to_string(), "Studienarbeit".to_string(), false),
                CourseResult::new("T3_1000".to_string(), "Praxisprojekt I".to_string(), false),
                CourseResult::new("T3_2000".to_string(), "Praxisprojekt II".to_string(), false),
                CourseResult::new("T3_3000".to_string(), "Praxisprojekt III".to_string(), false),
                CourseResult::new("T3INF4101".to_string(), "Web Engineering".to_string(), false),
                CourseResult::new("T3INF4103".to_string(), "Anwendungsprojekt Informatik".to_string(), false),
                CourseResult::new("T3INF4305".to_string(), "Softwarequalität und Verteilte Systeme".to_string(), false),
                CourseResult::new("T3INF4304".to_string(), "Datenbanken II".to_string(), false),
                CourseResult::new("T3_3300".to_string(), "Bachelorarbeit".to_string(), false),
            ]
        );
    }

    #[test]
    fn test_diff_results() {
        let old = vec![
            CourseResult::new("1".to_string(), "Test".to_string(), false),
            CourseResult::new("2".to_string(), "Test".to_string(), false),
            CourseResult::new("3".to_string(), "Test".to_string(), false),
            CourseResult::new("4".to_string(), "Test".to_string(), false),
            CourseResult::new("5".to_string(), "Test".to_string(), false),
        ];
        let new = vec![
            CourseResult::new("1".to_string(), "Test".to_string(), true),
            CourseResult::new("2".to_string(), "Test".to_string(), false),
            CourseResult::new("3".to_string(), "Test".to_string(), false),
            CourseResult::new("4".to_string(), "Test".to_string(), false),
            CourseResult::new("5".to_string(), "Test".to_string(), false),
        ];
        let changed = diff_results(&old, &new);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0], &CourseResult::new("1".to_string(), "Test".to_string(), true));
    }
}
