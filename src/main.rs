use dotenv::dotenv;
use reqwest::blocking::{Client, ClientBuilder};

const BASE_URL: &str = "https://dualis.dhbw.de";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv()?;

    let client = ClientBuilder::new().cookie_store(true).build()?;
    login(&client)?;

    Ok(())
}

fn login(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let login_url = format!("{}/scripts/mgrqispi.dll", BASE_URL);

    let username = std::env::var("DUALIS_EMAIL")?;
    let password = std::env::var("DUALIS_PASSWORD")?;
    let login_form: &[(&str, &str)] = &[
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

    let response = client.post(login_url).form(login_form).send()?;

    // Response code should always be 200. If the response body is too large,
    // it usually means that the login failed because a html page with a error is returned.
    let status = response.status();
    if !status.is_success() || response.text()?.len() > 500 {
        return Err(format!(
            "Login failed. Please check your credentials. Status code: {}",
            status
        )
        .into());
    }

    println!("Login successful!");

    Ok(())
}
