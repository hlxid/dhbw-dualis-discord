use dotenv::dotenv;
use regex::Regex;
use reqwest::blocking::{Client, ClientBuilder};
use scraper::{Html, Selector, ElementRef};

const BASE_URL: &str = "https://dualis.dhbw.de";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv()?;

    let client = ClientBuilder::new().cookie_store(true).build()?;
    let auth_arguments = login(&client)?;
    let result_html = fetch_results(&client, &auth_arguments)?;
    let results = parse_results(&result_html);

    for result in results {
        println!("Id: {}, Name: {}, scored: {}", result.course_id, result.course_name, result.scored);
    }

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
        .unwrap()
        .to_str()?
        .to_string();
    let content = response.text()?;

    if !status.is_success() || content.len() > 500 {
        return Err(format!(
            "Login failed. Please check your credentials. Status code: {}",
            status
        )
        .into());
    }

    println!("Login successful!");

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

#[derive(Debug)]
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
    println!("Successfully Parsed results.");

    let mut results = Vec::new();
    let course_name_replace_regex = Regex::new("<!--.+-->").unwrap();
    
    let table_rows_selector = Selector::parse("tbody tr").unwrap();
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

        if cells.iter().any(|cell| !cell.value().attr("class").unwrap_or_default().contains("tbdata")) {
            continue;
        }

        // Initial parsing:
        let course_id: String = cells[0].text().collect();
        let course_name: String = cells[1].text().map(|text_part| text_part.trim()).collect();
        let scored = cells[5].value().attr("title").unwrap_or("offen").to_lowercase() != "offen";

        // Value fixing:
        // Dualis is so bad that they use xml/html comments inside a script tag LMAO
        // Replace line endings so everything is a single line for the regex.
        let course_name = course_name.replace('\n', "");
        let course_name= course_name_replace_regex.replace_all(&course_name, "").to_string();
        
        let course_result = CourseResult::new(course_id, course_name, scored);
        results.push(course_result);
    }

    results
}

