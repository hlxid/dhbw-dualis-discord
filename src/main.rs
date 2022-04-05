use std::collections::HashMap;

use dotenv::dotenv;
use regex::Regex;
use reqwest::blocking::{Client, ClientBuilder};

use scraper::{ElementRef, Html, Selector};

mod results;
use results::{diff_results, load_results, save_results, CourseResult};

const BASE_URL: &str = "https://dualis.dhbw.de";

struct Semester {
    id: String,
    name: String,
}

fn login(client: &Client) -> Result<String, Box<dyn std::error::Error>> {
    println!("Logging in...");
    let url = format!("{BASE_URL}/scripts/mgrqispi.dll");

    let username = std::env::var("DUALIS_EMAIL")?;
    let password = std::env::var("DUALIS_PASSWORD")?;
    let form_data = [
        ("usrname", username.as_str()),
        ("pass", password.as_str()),
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

    let response = client.post(url).form(&form_data).send()?;

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
        return Err(
            format!("Login failed. Please check your credentials. Status code: {status}").into(),
        );
    }

    println!("Login successful!");

    let refresh_header = refresh_header?.to_str()?[84..] // TODO: unuglify this constant substring
        .to_string()
        .replace("-N000000000000000", "");

    Ok(refresh_header)
}

fn fetch_overview(
    client: &Client,
    auth_arguments: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    println!("Fetching overview...");
    let url = format!("{BASE_URL}/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=COURSERESULTS&ARGUMENTS={auth_arguments}");

    let response = client.get(url).send()?;
    let status = response.status();
    let content = response.text()?;

    if !status.is_success() || content.len() < 500 {
        return Err("Failed to fetch overview.".into());
    }

    println!("Successfully fetched overview!");

    Ok(content)
}

fn fetch_semester_details(
    client: &Client,
    auth_arguments: &str,
    semester: &Semester,
) -> Result<String, Box<dyn std::error::Error>> {
    println!(
        "Fetching result details of semester {} ({})...",
        semester.name, semester.id
    );

    let url = format!("{BASE_URL}/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=COURSERESULTS&ARGUMENTS={auth_arguments}-N{}", semester.id);

    let response = client.get(url).send()?;
    let status = response.status();
    let content = response.text()?;

    if !status.is_success() || content.len() < 500 {
        return Err("Failed to fetch result details.".into());
    }

    println!(
        "Successfully fetched result details of semester {}!",
        semester.name
    );

    Ok(content)
}

fn fetch_course_results(client: &Client, path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("{BASE_URL}{path}");
    let response = client.get(url).send()?;
    let status = response.status();
    let content = response.text()?;

    if !status.is_success() || content.len() < 500 {
        return Err("Failed to fetch course results.".into());
    }

    Ok(content)
}

fn parse_semesters(overview_html: &str) -> Vec<Semester> {
    println!("Parsing semesters...");
    let document = Html::parse_document(overview_html);

    let semester_selector = Selector::parse("select#semester").unwrap();
    let semester_options_selector = Selector::parse("option").unwrap();
    let semester_select = document.select(&semester_selector).next();

    let semesters = if let Some(semester_select) = semester_select {
        semester_select
            .select(&semester_options_selector)
            .map(|semester_option| {
                let id = semester_option.value().attr("value").unwrap_or_default();
                let name = semester_option.text().collect();
                Semester {
                    id: id.into(),
                    name,
                }
            })
            .collect()
    } else {
        vec![]
    };

    println!("Successfully parsed {} semesters!", semesters.len());
    semesters
}

fn parse_semester_details(details_html: &str) -> Vec<String> {
    let document = Html::parse_document(details_html);

    let selector = Selector::parse("td.tbdata a").unwrap();
    let a_tags = document.select(&selector);

    a_tags
        .filter_map(|tag| tag.value().attr("href"))
        .map(|s| s.to_string())
        .collect()
}

fn parse_course_results(results_html: &str) -> Vec<CourseResult> {
    // Selectors/Regex
    let table_rows_selector = Selector::parse("table tr").unwrap();
    let cell_selector = Selector::parse("td").unwrap();
    let h1_selector = Selector::parse("h1").unwrap();
    let course_id_regex = Regex::new(r"[A-Z0-9]+[0-9]{4}(\.[0-9]{1,2})?").unwrap();

    let document = Html::parse_document(results_html);
    let mut results = Vec::new();

    let main_course_name_full: String = document
        .select(&h1_selector)
        .next()
        .unwrap()
        .text()
        .collect();
    let main_course_name_full = main_course_name_full.replace('\n', "").trim().to_owned();
    let main_course_id = course_id_regex
        .find(&main_course_name_full)
        .unwrap()
        .as_str()
        .trim()
        .to_owned();
    let main_course_name = course_id_regex
        .replace_all(&main_course_name_full, "")
        .trim()
        .to_owned();

    let mut sub_course_id = None;
    let mut sub_course_name = String::default();

    let table_rows = document.select(&table_rows_selector);
    for row in table_rows {
        // Filter useless rows
        let row_classes = row.value().attr("class").unwrap_or_default();
        if row_classes.contains("subhead") || row_classes.contains("level00") {
            continue;
        }

        let cells: Vec<ElementRef> = row.select(&cell_selector).collect();

        if cells.len() == 1
            && cells[0]
                .value()
                .attr("class")
                .unwrap_or_default()
                .contains("level02")
        {
            let name: String = cells[0].text().collect();
            sub_course_id = course_id_regex
                .find(&name)
                .map(|s| s.as_str().trim().to_owned());
            sub_course_name = course_id_regex.replace_all(&name, "").trim().to_owned();

            continue;
        }

        if cells.len() < 6 {
            continue;
        }

        // Cells that don't have the tbdata class are not relevant
        if cells.iter().any(|cell| {
            !cell
                .value()
                .attr("class")
                .unwrap_or_default()
                .contains("tbdata")
        }) {
            continue;
        }

        // Parsing
        let course_id = sub_course_id
            .clone()
            .unwrap_or_else(|| main_course_id.clone());
        let course_name = if sub_course_name == "Modulabschlussleistungen" {
            main_course_name.clone()
        } else {
            sub_course_name.clone()
        };

        let points: String = cells[3].text().collect();
        let scored = !points.is_empty() && !points.contains("noch nicht");

        let course_result = CourseResult::new(course_id, course_name, scored);
        results.push(course_result);
    }

    results
}

fn get_course_results(
    client: &Client,
    auth_arguments: &str,
) -> Result<Vec<CourseResult>, Box<dyn std::error::Error>> {
    let overview_html = fetch_overview(client, auth_arguments)?;
    let semesters = parse_semesters(&overview_html);
    let mut results = vec![];

    for semester in semesters.iter() {
        println!("Fetching semester {}...", semester.name);
        let details_html = fetch_semester_details(client, auth_arguments, semester)?;

        let course_urls = parse_semester_details(&details_html);
        for course_path in course_urls.iter() {
            let course_html = fetch_course_results(client, course_path)?;
            let course_results = parse_course_results(&course_html);

            results.extend(course_results);
        }
    }

    // make sure that results are unique by id
    let mut unique_results = vec![];
    for result in results.iter() {
        if !unique_results
            .iter()
            .any(|r: &CourseResult| r.course_id == result.course_id)
        {
            unique_results.push(result.clone());
        }
    }

    Ok(unique_results)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv()?;

    let client = ClientBuilder::new().cookie_store(true).build()?;
    let auth_arguments = login(&client)?;

    let results = get_course_results(&client, &auth_arguments)?;

    for result in results.iter() {
        println!("{result}");
    }

    let old_results = load_results();
    if let Some(old_results) = old_results {
        let changes = diff_results(&old_results, &results);
        for change in changes {
            handle_newly_scored_course(&client, change)
        }
    } else {
        println!("No saved results found. Not looking for changes.");
    }

    save_results(&results)?;

    Ok(())
}

fn handle_newly_scored_course(client: &Client, result: &CourseResult) {
    println!("Newly scored course: {}", result);

    // get webhook url or print a warning message that it is missing or empty
    let webhook_url = std::env::var("DISCORD_WEBHOOK").ok();
    if webhook_url.is_none() || webhook_url.as_ref().unwrap().is_empty() {
        println!("DISCORD_WEBHOOK is not set. Not sending webhook.");
        return;
    }
    let webhook_url = webhook_url.unwrap();

    // Send discord webhook request
    let mut payload = HashMap::new();
    payload.insert("content", format!("Neue Ergebnise in Dualis eingetragen: {} ({})", result.course_name, result.course_id));

    let response = client.post(&webhook_url).json(&payload).send().unwrap();

    if !response.status().is_success() {
        panic!(
            "Error sending discord webhook: {}",
            response.text().unwrap()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_semester_details() {
        let html = include_str!("../test_data/semester_details.html");
        let course_urls = parse_semester_details(html);

        assert_eq!(course_urls.len(), 8);
        assert_eq!(course_urls, vec![
            "/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=RESULTDETAILS&ARGUMENTS=-N796098644273095,-N000019,-N380913492536419,-N000000015098000",
            "/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=RESULTDETAILS&ARGUMENTS=-N796098644273095,-N000019,-N381934466869103,-N000000015098000",
            "/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=RESULTDETAILS&ARGUMENTS=-N796098644273095,-N000019,-N380913840065009,-N000000015098000",
            "/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=RESULTDETAILS&ARGUMENTS=-N796098644273095,-N000019,-N380914243305413,-N000000015098000",
            "/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=RESULTDETAILS&ARGUMENTS=-N796098644273095,-N000019,-N381934623749891,-N000000015098000",
            "/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=RESULTDETAILS&ARGUMENTS=-N796098644273095,-N000019,-N382213482644004,-N000000015098000",
            "/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=RESULTDETAILS&ARGUMENTS=-N796098644273095,-N000019,-N380914104617007,-N000000015098000",
            "/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=RESULTDETAILS&ARGUMENTS=-N796098644273095,-N000019,-N380914015873077,-N000000015098000",
        ]);
    }

    #[test]
    fn test_parse_course_results_single() {
        let html = include_str!("../test_data/result_details_single.html");
        let results = parse_course_results(html);

        assert_eq!(
            results,
            vec![CourseResult {
                course_id: "T3INF1002".into(),
                course_name: "Theoretische Informatik I (WiSe 2021/22)".into(),
                scored: true,
            },]
        );
    }

    #[test]
    fn test_parse_course_results_multiple() {
        let html = include_str!("../test_data/result_details_multiple.html");
        let results = parse_course_results(html);

        assert_eq!(
            results,
            vec![
                CourseResult {
                    course_id: "T3INF1001.1".into(),
                    course_name: "Lineare Algebra (MOS-TINF21B)".into(),
                    scored: false,
                },
                CourseResult {
                    course_id: "T3INF1001.2".into(),
                    course_name: "Analysis (MOS-TINF21B)".into(),
                    scored: false,
                }
            ]
        );
    }
}
