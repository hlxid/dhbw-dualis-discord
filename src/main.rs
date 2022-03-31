use dotenv::dotenv;
use reqwest::blocking::{Client, ClientBuilder};

const BASE_URL: &str = "https://dualis.dhbw.de";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv()?;

    let client = ClientBuilder::new().cookie_store(true).build()?;
    let auth_arguments = login(&client)?;
    let results = fetch_results(&client, &auth_arguments)?;

    println!("{}", results);

    Ok(())
}

fn login(client: &Client) -> Result<String, Box<dyn std::error::Error>> {
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
    let refresh_header = response.headers().get("REFRESH").unwrap().to_str()?.to_string();
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

fn fetch_results(client: &Client, auth_arguments: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("{}/scripts/mgrqispi.dll?APPNAME=CampusNet&PRGNAME=STUDENT_RESULT&ARGUMENTS={}", BASE_URL, auth_arguments);

    let response = client.get(url).send()?;
    let status = response.status();
    let content = response.text()?;

    if !status.is_success() || content.len() < 500 {
        return Err("Failed to fetch results.".into())
    }

    Ok(content)
}
