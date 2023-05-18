## Appstore Bot for DeltaChat
Welcome to the official repository for the DeltaChat Appstore Bot. You can start using it today by contacting it at this `<email>`. The appstore bot acts both as distribution platform as well as publishing service.

### Using the Appstore Bot
**Downloading Apps**: When sending a message to the bot, it will reply with the appstore webxdc. You can then clicke the `add` button and the bot will send you the requested webxdc.

**Publishing Apps**: The `appstore` also provides a platform for developers to publish their own applications. Here's a step-by-step guide on how to it:

1. Navigate to the publish page within the appstore.
2. Provide some basic information about your app, such as the name and description.
3. Upon submission, the bot creation a new review group.
   - This group comprises multiple testers who test the application across various devices.
   - Additionally, the group contains one reviewer who, will publish the app if it meets all requirements.
4. Post your bundled webxdc into the review group.
5. Provide any additional necessary information to meet the testers' requirements.
6. Once all requirements are met, the publisher will publish your app to the appstore.

### App Publishing Requirements:
The following structure outlines the essential information needed for publishing an app:

```rust
Copy code
pub struct AppInfo {
    pub name: String,                    // Taken from manifest.toml
    pub author_name: String,             // Generated by bot from contact
    pub author_email: Option<String>,    // Generated by bot from contact
    pub source_code_url: Option<String>, // Taken from manifest.toml
    pub image: Option<String>,           // Taken from manifest.toml
    pub description: String,             // Provided in the submission form
    pub xdc_blob_dir: Option<PathBuf>,   // Generated by bot from contact
    pub version: Option<String>,         // Taken from manifest.toml
}
```

### Setting Up the Bot
To get started with the bot, clone this repository and initiate the bot using the following command:

```
addr="<email>" mail_pw="<password>" RUST_LOG=info cargo r
```

**Attention**: you also need to build the frontend by going to the folder, installing all npm packages and executing the build script. At the end you need to run the `./create_xdc.sh` skript which will create the `appstore-bot.xdc` file which will be distributed by the bot.

On activation, the bot will prompt you for an administrator email address. Upon providing this, the bot will establish two groups: the `Publisher Group` and the `Tester Group`.

- `Publisher Group`: This group consists of trusted entities authorized to add an app to the appstore.
- `Tester Group`: A collection of testers, possibly from the community, who are capable of testing the apps on their devices.

To assign new members to these roles, simply add them to the respective group chats.

## Development

The used database is surrealdb. You can run a local serve like this 
```
surreal start --log trace --user root file://bot.db
```
and use some client like `Insomnia` to the sql backend `localhost:8000/sql`.