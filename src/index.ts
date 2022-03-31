console.log("Hello world");

// Import env vars from configuration file.
import * as dotenv from "dotenv";
dotenv.config();

console.log(process.env.DUALIS_EMAIL);
