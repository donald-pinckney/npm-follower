const ChangesStream = require('changes-stream');
const Request = require('request');
const fs = require('fs');
const yargs = require('yargs/yargs');
const { hideBin } = require('yargs/helpers');
const path = require('path');

const argv = yargs(hideBin(process.argv)).argv
const changes_root_path = argv.changes_path_root;

if(!changes_root_path) {
  console.log('Please specify a path for the root of the destination changes directory');
  process.exit(1);
}


const seq_path = path.join(changes_root_path, 'sequence.json');
const log_path = path.join(changes_root_path, 'log.jsonl');


let since_when;
if(fs.existsSync(seq_path)) {
  since_when = JSON.parse(fs.readFileSync(seq_path));
} else {
  since_when = null;
}


const db = 'https://replicate.npmjs.com';

let changes;
if(since_when != null) {
  changes = new ChangesStream({
    db: db,
    include_docs: true,
    since: since_when
  });
} else {
  changes = new ChangesStream({
    db: db,
    include_docs: true,
  });
}




const log_fd = fs.openSync(log_path, 'a');

function writeSeqToFile(seq) {
  const seq_fd = fs.openSync(seq_path, 'w');
  fs.writeSync(seq_fd, JSON.stringify(seq));
  fs.fsyncSync(seq_fd);
  fs.closeSync(seq_fd);
}



Request.get(db, function(err, req, body) {
  const end_sequence = JSON.parse(body).update_seq;

  console.log('Starting replication for range: (', since_when == null ? 'start-of-time' : since_when, ', ', end_sequence, ']');

  changes.on('data', function(change) {

    const seq = change.seq;

    fs.writeSync(log_fd, JSON.stringify(change) + '\n');
    fs.fsyncSync(log_fd);

    writeSeqToFile(seq);

    console.log("Wrote log entry for sequence: ", seq);

    if (seq >= end_sequence) {
      process.exit(0);
    }
  });
});

