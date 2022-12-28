# run in psql:  create database npm_data;

# run in psql -d npm_data
# ALTER DEFAULT PRIVILEGES 
#     IN SCHEMA public
#     GRANT ALL TO pinckney;


# pg_restore -d npm_data -e -O -j 4 /home/pinckney/db_backups/DATE/

#grant all on schema public to pinckney;
#grant all on schema public to federico;
#alter default privileges in schema public grant all on tables to pinckney, federico;
#alter default privileges in schema public grant all on tables to federico;
#alter default privileges in schema public grant all on sequences to pinckney, federico;
#alter default privileges in schema public grant all on functions to pinckney, federico;
#alter default privileges in schema public grant all on types to pinckney, federico;
#alter default privileges grant all on schemas to pinckney, federico;