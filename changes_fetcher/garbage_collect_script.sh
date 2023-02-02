#!/bin/bash

# deletes everything except for the last row in the "change_log" table

# ask for confirmation
echo "Are you sure you want to garbage collect the change_log table?"
echo "This operation should only be done after you have completely scraped all data from the table."
echo "This means you should have done: "
echo "1. Queued and downloaded all tarballs"
echo "2. Ran the diff log builder"
echo "3. Ran the relational db builder"
echo "THIS OPERATION CANNOT BE UNDONE."
echo "Type 'YES' to continue, or anything else to cancel."
read answer
if [ "$answer" != "YES" ]; then
    echo "Deleting all but the last row in the change_log table..."
else
    echo "Aborting..."
    exit 1
fi

# get path to this script
SCRIPT_PATH=$(dirname $(readlink -f $0))
# cd to parent of this script
cd $SCRIPT_PATH/..

# read the .env file
export $(grep -v '^#' .env | xargs)

psql -h $DATABASE_HOST -p $DATABASE_PORT -U $DATABASE_USER -d $DATABASE_NAME \
  -c "DELETE FROM change_log WHERE seq NOT IN (SELECT seq FROM change_log ORDER BY seq DESC LIMIT 1);"
