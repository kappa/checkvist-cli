use crate::api::{CheckvistApi, Order};
use crate::cfg::ConfigLoader;
use crate::cli::{AuthCommand, Cli, Commands, ListsCommand};
use crate::error::{AppError, AppResult, ErrorKind};
use crate::token_store;
use crate::output::{print_auth_status, print_lists};

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
    let config = loader.load(
        profile_override,
        base_url_override,
        cli.auth_file.clone(),
        cli.token_file.clone(),
    )?;

    let api = CheckvistApi::new(config.base_url.clone());

    match cli.command {
        Some(Commands::Lists(ListsCommand::Get(args))) => {
            let order = match args.order.as_deref() {
                Some("id:asc") => Some(Order::IdAsc),
                Some("id:desc") => Some(Order::IdDesc),
                Some("updated_at:asc") => Some(Order::UpdatedAtAsc),
                Some(other) => {
                    return Err(AppError::new(
                        ErrorKind::Argument,
                        format!("invalid order: {}", other),
                    ))
                }
                None => None,
            };

            let token = ensure_token(&api, &config)?;
            let login = || api.login(&config.username, &config.remote_key, config.token2fa.as_deref());
            let lists = request::with_token_retry(
                &api,
                &config.token_file,
                token,
                |api, token| api.get_checklists(token, Some(args.archived), order, Some(args.skip_stats)),
                login,
            )?;

            print_lists(&lists, cli.format)?;
            Ok(())
        }
        Some(Commands::Auth(AuthCommand::Status(args))) => {
            let token = ensure_token(&api, &config)?;
            let login = || api.login(&config.username, &config.remote_key, config.token2fa.as_deref());
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
        None => Err(AppError::new(ErrorKind::Argument, "no command provided")),
    }
}

fn ensure_token(api: &CheckvistApi, config: &crate::cfg::AuthConfig) -> AppResult<String> {
    if let Some(token) = token_store::read_token(&config.token_file)? {
        return Ok(token);
    }

    let token = api.login(&config.username, &config.remote_key, config.token2fa.as_deref())?;
    token_store::write_token(&config.token_file, &token)?;
    Ok(token)
}
