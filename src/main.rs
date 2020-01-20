mod csv;
mod fetch;
mod pull_request;

use clap::{App, Arg};
use github_gql::client::Github;
use serde_json::Value;
use std::collections::HashMap;

use fetch::{fetch_teams, pull_requests};

fn index_teams_by_users(teams: &Vec<Value>) -> HashMap<&str, Vec<&str>> {
    let mut index = HashMap::new();
    for team in teams {
        let team_name = team["name"].as_str().unwrap();
        let members: Vec<_> = team["members"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|member| member["login"].as_str().unwrap())
            .collect();

        for member in members {
            let members_teams = index.entry(member).or_insert(Vec::new());
            members_teams.push(team_name);
        }
    }

    return index;
}

fn main() {
    let matches = App::new("Pull Request Stats")
        .version("iv20-01-2020")
        .author("Viktor Charypar <charypar@gmail.com>")
        .about("Gives statistics for pull requests in a repository focusing on reviews")
        .arg(
            Arg::with_name("OWNER")
                .index(1)
                .required(true)
                .help("Repository owner"),
        )
        .arg(
            Arg::with_name("REPO")
                .index(2)
                .required(true)
                .help("Repository name"),
        )
        .arg(
            Arg::with_name("COUNT")
                .index(3)
                .help("Number of pull requests to fetch (default 100)"),
        )
        .arg(
            Arg::with_name("teams")
                .short("t")
                .long("teams")
                .takes_value(true)
                .help("Teams to consider"),
        )
        .get_matches();

    let org = matches.value_of("OWNER").expect("Specify owner!");
    let repo = matches.value_of("REPO").expect("Specify repo!");
    let limit = matches
        .value_of("COUNT")
        .unwrap_or("100")
        .parse::<usize>()
        .unwrap();
    let teams = matches.value_of("teams").unwrap_or("");

    let mut github = Github::new("92e86a66b4f38662fbb67d6560a419808d891b62").unwrap();

    let teams = fetch_teams(&mut github, org, teams);
    let team_from_user = index_teams_by_users(&teams);

    csv::print_header();
    for batch in pull_requests(&mut github, org, repo, limit) {
        for json in batch {
            csv::print_pr(pull_request::build(&json, &team_from_user))
        }
    }
}
