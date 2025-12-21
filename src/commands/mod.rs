use crate::api::{CheckvistApi, Order};
use crate::cfg::{ConfigLoader, ResolvedConfig};
use crate::cli::{AuthCommand, AuthLoginArgs, Cli, Commands, ListsCommand};
use crate::error::{AppError, AppResult, ErrorKind};
use crate::output::{print_auth_status, print_lists};
use crate::token_store;
use crate::{cfg, cli};
use clap::CommandFactory;
use std::io::{self, Write};

pub mod request;

pub fn dispatch(cli: Cli) -> AppResult<()> {
    let loader = ConfigLoader::new();
    let profile_override = if cli.profile == "default" {
        None
    } else {
        Some(cli.profile.clone())
    };
    let base_url_override = if cli.base_url == "https://checkvist.com" {
        None
    } else {
        Some(cli.base_url.clone())
    };
    match cli.command {
        Some(Commands::Lists(ListsCommand::Get(args))) => {
            let config = loader.load(
                profile_override,
                base_url_override.clone(),
                cli.auth_file.clone(),
                cli.token_file.clone(),
            )?;
            let api = CheckvistApi::new(config.base_url.clone());
            let order = match args.order.as_deref() {
                Some("id:asc") => Some(Order::IdAsc),
                Some("id:desc") => Some(Order::IdDesc),
                Some("updated_at:asc") => Some(Order::UpdatedAtAsc),
                Some(other) => {
                    return Err(AppError::new(
                        ErrorKind::Argument,
                        format!("invalid order: {}", other),
                    ));
                }
                None => None,
            };

            let token = ensure_token(&api, &config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let lists = request::with_token_retry(
                &api,
                &config.token_file,
                token,
                |api, token| {
                    api.get_checklists(token, Some(args.archived), order, Some(args.skip_stats))
                },
                login,
            )?;

            print_lists(&lists, cli.format)?;
            Ok(())
        }
        Some(Commands::Auth(AuthCommand::Status(args))) => {
            let config = loader.load(
                profile_override,
                base_url_override.clone(),
                cli.auth_file.clone(),
                cli.token_file.clone(),
            )?;
            let api = CheckvistApi::new(config.base_url.clone());
            let token = ensure_token(&api, &config)?;
            let login = || {
                api.login(
                    &config.username,
                    &config.remote_key,
                    config.token2fa.as_deref(),
                )
            };
            let user = request::with_token_retry(
                &api,
                &config.token_file,
                token,
                |api, token| api.auth_status(token),
                login,
            )?;

            print_auth_status(&user, args.format)?;
            Ok(())
        }
        Some(Commands::Auth(AuthCommand::Login(args))) => {
            let resolved = loader.resolve(
                profile_override,
                base_url_override,
                cli.auth_file.clone(),
                cli.token_file.clone(),
            );
            run_login(&resolved, &args)
        }
        None => Err(AppError::new(
            ErrorKind::Argument,
            cli::Cli::command().render_help().to_string(),
        )),
    }
}

fn ensure_token(api: &CheckvistApi, config: &crate::cfg::AuthConfig) -> AppResult<String> {
    if let Some(token) = token_store::read_token(&config.token_file)? {
        return Ok(token);
    }

    let token = api.login(
        &config.username,
        &config.remote_key,
        config.token2fa.as_deref(),
    )?;
    token_store::write_token(&config.token_file, &token)?;
    Ok(token)
}

fn run_login(resolved: &ResolvedConfig, _args: &AuthLoginArgs) -> AppResult<()> {
    let base = resolved.base_url.trim_end_matches('/');
    let login_url = format!("{}/auth/login", base);
    let api_key_url = format!("{}/auth/profile", base);

    println!(
        "We'll create credentials for profile \"{}\" in {}",
        resolved.profile,
        resolved.auth_file.display()
    );
    println!("1) Open and sign in: {login_url}");
    println!("2) Copy your Remote API key from: {api_key_url}");
    println!("3) Paste your login name and Remote API key below.");
    println!("(Press Enter to leave 2FA token empty if you don't use it.)\n");

    let mut stdout = io::stdout();
    let stdin = io::stdin();

    print!("Checkvist login/email: ");
    stdout.flush().ok();
    let mut username = String::new();
    stdin.read_line(&mut username).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!("failed to read username: {}", err),
        )
    })?;

    print!("Remote API key: ");
    stdout.flush().ok();
    let mut remote_key = String::new();
    stdin.read_line(&mut remote_key).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!("failed to read remote key: {}", err),
        )
    })?;

    print!("2FA token (optional): ");
    stdout.flush().ok();
    let mut token2fa = String::new();
    stdin.read_line(&mut token2fa).map_err(|err| {
        AppError::new(
            ErrorKind::Local,
            format!("failed to read 2FA token: {}", err),
        )
    })?;

    cfg::write_auth_config(
        &resolved.auth_file,
        &resolved.profile,
        username.trim(),
        remote_key.trim(),
        Some(token2fa.trim()).filter(|v| !v.is_empty()),
    )?;

    println!(
        "Saved credentials for profile \"{}\" to {}",
        resolved.profile,
        resolved.auth_file.display()
    );
    println!("Run `checkvist auth status` to verify your setup.");

    Ok(())
}
