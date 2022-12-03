### :warning: not in active use anymore and therefore not maintained. Could break in the future

# dhbw-dualis-discord

This is a tool that scrapes the [DUALIS](https://dualis.dhbw.de/) website of DHBW and posts a message into a Discord channel when the point results of a course are entered.

## Configuration

Copy the `.env.sample` file into a file named `.env` and fill in your DUALIS login information.

To get the required discord webhook url go to your Discord server settings > Integrations > Webhook and create one. You can customize the name and profile image of the bot there.

Once you have entered the required values you can build and run this tool using `cargo run --release` or using the following docker command:

```shell
$ docker run --rm -v $(pwd):/data ghcr.io/daniel0611/dhbw-dualis-discord
```

The current state will be saved inside a json file at `./dualis_results.json`.
This file will be used to check which courses are newly scored vs courses that already have their results published.
