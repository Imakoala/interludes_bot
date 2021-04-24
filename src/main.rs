use serenity::{
    async_trait,
    framework::standard::macros::*,
    model::{event::PresenceUpdateEvent, prelude::Ready},
};
use serenity::{
    client::bridge::gateway::GatewayIntents,
    client::{Client, Context, EventHandler},
    framework::standard::help_commands,
    framework::standard::{Args, CommandGroup, HelpOptions},
    model::id::UserId,
};
use serenity::{
    framework::standard::{
        macros::{command, group},
        CommandResult, StandardFramework,
    },
    prelude::TypeMapKey,
};
use serenity::{http::Http, model::channel::Message};

use std::{
    collections::{HashMap, HashSet},
    env,
    time::Instant,
};

#[group]
#[commands(add, whosonline)]
struct General;
struct CommandCounter;

impl TypeMapKey for CommandCounter {
    type Value = HashMap<String, u64>;
}

struct OnlineTracker;

impl TypeMapKey for OnlineTracker {
    type Value = HashMap<UserId, std::time::Instant>;
}
struct Handler;

#[async_trait]
impl EventHandler for Handler {
    // As the intents set in this example, this event shall never be dispatched.
    // Try it by changing your status.
    async fn presence_update(&self, ctx: Context, new_data: PresenceUpdateEvent) {
        let mut data = ctx.data.write().await;
        let tracker = data
            .get_mut::<OnlineTracker>()
            .expect("Expected CommandCounter in TypeMap.");
        let user_id = new_data.presence.user_id;
        use serenity::model::prelude::OnlineStatus::*;
        let online = match new_data.presence.status {
            DoNotDisturb | Idle | Invisible | Online => true,
            _ => false,
        };
        if online && !tracker.contains_key(&user_id) {
            tracker.insert(user_id, Instant::now());
        }
        if !online {
            tracker.remove(&user_id);
        }
    }
    async fn ready(&self, ctx: Context, ready: Ready) {
        let mut data = ctx.data.write().await;
        let now = Instant::now();
        let tracker = data
            .get_mut::<OnlineTracker>()
            .expect("Expected CommandCounter in TypeMap.");

        if let Some(guild) = ready.guilds[0].id().to_guild_cached(&ctx).await {
            println!("found guild {}", guild.name);
            *tracker = guild
                .presences
                .iter()
                .filter_map(|(id, presence)| {
                    use serenity::model::prelude::OnlineStatus::*;
                    let online = match presence.status {
                        DoNotDisturb | Idle | Invisible | Online => true,
                        _ => false,
                    };
                    if online {
                        Some((*id, now.clone()))
                    } else {
                        None
                    }
                })
                .collect();
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("token");
    let http = Http::new_with_token(&token);
    let bot_id = match http.get_current_user().await {
        Ok(bot_id) => bot_id.id,
        Err(why) => panic!("Could not access the bot id: {:?}", why),
    };

    let framework = StandardFramework::new()
        .configure(|c| {
            c.prefix("!")
                .with_whitespace(true)
                .on_mention(Some(bot_id))
                .prefix("!")
                .delimiters(vec![", ", ","])
        })
        .group(&GENERAL_GROUP)
        .before(before)
        .after(after)
        .unrecognised_command(unknown_command)
        .help(&MY_HELP);

    // Login with a bot token from the environment
    let mut client = Client::builder(token)
        .event_handler(Handler)
        .intents(GatewayIntents::all())
        .framework(framework)
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<CommandCounter>(HashMap::default());
        data.insert::<OnlineTracker>(HashMap::default());
    }
    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

#[help]
#[command_not_found_text = "Could not find: `{}`."]
#[max_levenshtein_distance(3)]
#[lacking_permissions = "Strike"]
#[lacking_role = "Strike"]
#[wrong_channel = "Strike"]
async fn my_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
}
#[hook]
async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
    println!(
        "Got command '{}' by user '{}'",
        command_name, msg.author.name
    );

    // Increment the number of times this command has been run once. If
    // the command's name does not exist in the counter, add a default
    // value of 0.
    let mut data = ctx.data.write().await;
    let counter = data
        .get_mut::<CommandCounter>()
        .expect("Expected CommandCounter in TypeMap.");
    let entry = counter.entry(command_name.to_string()).or_insert(0);
    *entry += 1;

    true // if `before` returns false, command processing doesn't happen.
}

#[hook]
async fn after(_ctx: &Context, _msg: &Message, command_name: &str, command_result: CommandResult) {
    match command_result {
        Ok(()) => println!("Processed command '{}'", command_name),
        Err(why) => println!("Command '{}' returned error {:?}", command_name, why),
    }
}

#[hook]
async fn unknown_command(_ctx: &Context, _msg: &Message, unknown_command_name: &str) {
    println!("Could not find command named '{}'", unknown_command_name);
}

#[command]
async fn add(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "Pong!").await?;

    Ok(())
}

#[command]
async fn whosonline(ctx: &Context, msg: &Message) -> CommandResult {
    let mut data = ctx.data.write().await;
    let tracker = data
        .get_mut::<OnlineTracker>()
        .expect("Expected CommandCounter in TypeMap.");
    let mut reply = "the following users are online:\n".to_string();
    for (userid, instant) in tracker.iter() {
        let member = msg
            .guild_id
            .ok_or("Must be used in guild")?
            .member(ctx, userid)
            .await?;
        let duration = instant.elapsed();
        let seconds = duration.as_secs() % 60;
        let minutes = (duration.as_secs() / 60) % 60;
        let hours = (duration.as_secs() / 60) / 60;
        reply += &format!(
            "{} has been connected for {}h {}m {}s\n",
            member.display_name(),
            hours,
            minutes,
            seconds
        );
    }
    msg.reply(ctx, reply).await?;
    Ok(())
}
