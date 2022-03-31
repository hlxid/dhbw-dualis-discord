use dotenv::dotenv;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv()?;
    
    println!("Hello, world!");
    println!("{}", std::env::var("DUALIS_EMAIL")?);

    Ok(())
}
