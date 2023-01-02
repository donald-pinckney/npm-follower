#!/bin/bash

# this program takes in two paths to npm tarballs and prints out the paths to every file
# and the number of lines added and removed in each file. then list out the total number of
# lines for each tarball. also list the average line width for each tarball.
# for example, an output would be:
# package.json 0 0 20 20 3.4 3.4
# index.js 10 0 20 30 4.4 2.3
# lib/index.js 0 10 20 10 9.4 1.2
#
# this means that package.json was not changed, index.js had 10 lines added, and lib/index.js had 10 lines removed

OLD=$1
NEW=$2

# extract the tarballs in the directory that they are in
tar -xzf $OLD -C $(dirname $OLD)
tar -xzf $NEW -C $(dirname $NEW)

DIR_OLD=$(dirname $OLD)/package
DIR_NEW=$(dirname $NEW)/package

# find all the files in the tarballs
# only get "js,ts,jsx,tsx,json,wat,wast" files
FILES_OLD=$(find $DIR_OLD -type f -name "*.js" -o -name "*.ts" -o -name "*.jsx" -o -name "*.tsx" -o -name "*.json" -o -name "*.wat" -o -name "*.wast")
FILES_NEW=$(find $DIR_NEW -type f -name "*.js" -o -name "*.ts" -o -name "*.jsx" -o -name "*.tsx" -o -name "*.json" -o -name "*.wat" -o -name "*.wast")

# if we have more than 200 files in either, bail with "ERROR: TOO MANY FILES"
# code 103
if [ $(echo "$FILES_OLD" | wc -l) -gt 200 ] || [ $(echo "$FILES_NEW" | wc -l) -gt 200 ]; then
    echo "ERROR: TOO MANY FILES"
    exit 103
fi

# for each file, diff and count the number of '<' and '>'
# the number of '<' is the number of lines removed
# the number of '>' is the number of lines added
for file in $FILES_OLD; do
    if [[ -f $file ]]; then
        file_new=$(echo $file | sed "s|$DIR_OLD|$DIR_NEW|")
        display_name=$(echo $file | sed "s|$DIR_OLD/||")
        if [[ -f $file_new ]]; then
            diff=$(diff $file $file_new)
            added=$(echo "$diff" | grep '^>' | wc -l)
            removed=$(echo "$diff" | grep '^<' | wc -l)
            l_old=$(wc -l $file | awk '{print $1}')
            l_new=$(wc -l $file_new | awk '{print $1}')
            w_old=$(awk '{print length}' $file 2> /dev/null | awk '{sum+=$1} END {print sum/NR}' 2> /dev/null)
            w_new=$(awk '{print length}' $file_new 2> /dev/null | awk '{sum+=$1} END {print sum/NR}' 2> /dev/null)
            echo $display_name $added $removed $l_old $l_new $w_old $w_new
        else
            echo $display_name 0 0 $(wc -l $file | awk '{print $1}') X $(awk '{print length}' $file 2> /dev/null | awk '{sum+=$1} END {print sum/NR}' 2> /dev/null) 0
        fi
    fi
done

# for each file in the new tarball, if it doesn't exist in the old tarball, it was added
for file in $FILES_NEW; do
    if [[ -f $file ]]; then
        file_old=$(echo $file | sed "s|$DIR_NEW|$DIR_OLD|")
        display_name=$(echo $file | sed "s|$DIR_NEW/||")
        if [[ ! -f $file_old ]]; then
            lines=$(wc -l < $file)
            width=$(awk '{print length}' $file 2> /dev/null | awk '{sum+=$1} END {print sum/NR}' 2> /dev/null)
            echo $display_name 0 0 0 $lines X $width
        fi
    fi
done

# clean up the extracted tarballs
rm -rf $DIR_OLD
rm -rf $DIR_NEW
