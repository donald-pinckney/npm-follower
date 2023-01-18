# run in psql:  create database npm_data;

# run in psql -d npm_data
# ALTER DEFAULT PRIVILEGES 
#     IN SCHEMA public
#     GRANT ALL TO XXXXXX;


# pg_restore -d npm_data -e -O -j 4 /home/XXXXXX/db_backups/DATE/

#grant all on schema public to XXXXXX;
#alter default privileges in schema public grant all on tables to XXXXXX;
#alter default privileges in schema public grant all on sequences to XXXXXX;
#alter default privileges in schema public grant all on functions to XXXXXX;
#alter default privileges in schema public grant all on types to XXXXXX;
#alter default privileges grant all on schemas to XXXXXX;