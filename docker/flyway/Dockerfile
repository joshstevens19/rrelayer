FROM flyway/flyway:6.3.1

COPY /rrelayerr_server/schema/* /flyway/sql/

CMD ["-X", "migrate", "-baselineOnMigrate=false", '-defaultSchema="public"', '-schemas="public"']
