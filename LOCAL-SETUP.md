# Installed before

- Must have cargo installed with rust - https://www.rust-lang.org/tools/install
- Must have docker installed 

# Server

- create a file called .env in the `rrelayer_server` directory paste the content josh sent you
- create a file called `setup.yaml` in the `rrelayer_server` directory paste the content josh sent you
  but add a wallet you own in `admins`
- open up a new terminal in `rrelayer_server`
- run docker-compose up
- open up a new terminal in `rrelayer_server`
- run `cargo run`
- API should be running

# Dashboard

- open up terminal in `rrelayer_dashboard`
- run `npm install`
- run `npm run dev`
- open up `http://localhost:3000/login` in your browser
- you are in
