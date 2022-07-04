const npa = require("npm-package-arg");
const semver = require("semver");
const net = require("net");

if (process.argv.length != 4) {
  console.log("usage: [path to socket] [pid of rust proc]");
  process.exit(1);
}

// every 3 seconds, check if the rust proc is still alive, if not we quit
const rustpid = process.argv[3];
setInterval(function () {
  if (!pidIsRunning(rustpid)) {
    process.exit(0);
  }
}, 3000);

var unixServer = net.createServer(function (client) {
  client.on("data", function (data) {
    try {
      const parsed = parse_spec(data.toString()) + "\n";
      client.write(parsed);
    } catch (e) {
      client.write(
        JSON.stringify({ Invalid: e.code + ": " + e.message }) + "\n"
      );
    }
  });
});

const socket = process.argv[2];
unixServer.listen(socket);
console.log("Listening on " + socket + "\n");

// handlers to close the server, or the socket will remain open forever
process.on("exit", close);
process.on("SIGINT", close);
process.on("SIGTERM", close);

function close() {
  console.log("Closing " + socket);
  unixServer.close();
}

function parse_spec(s) {
  const raw_spec = s.trim();

  const parsed_spec = npa.resolve("foo", raw_spec);
  const type = parsed_spec.type;

  let answer = null;
  if (type == "git") {
    answer = { Git: parsed_spec.saveSpec };
  } else if (type == "version" || type == "range") {
    answer = { Range: parse_range(parsed_spec.fetchSpec) };
  } else if (type == "tag") {
    answer = { Tag: parsed_spec.fetchSpec };
  } else if (type == "file") {
    answer = { File: remove_prefix(parsed_spec.rawSpec, "file:") };
  } else if (type == "directory") {
    answer = { Directory: remove_prefix(parsed_spec.rawSpec, "file:") };
  } else if (type == "remote") {
    answer = { Remote: parsed_spec.rawSpec };
  } else if (type == "alias") {
    const sub_type = parsed_spec.subSpec.type;
    let sub_answer = null;
    if (sub_type == "tag") {
      sub_answer = { Tag: parsed_spec.subSpec.fetchSpec };
    } else if (sub_type == "version" || sub_type == "range") {
      sub_answer = { Range: parse_range(parsed_spec.subSpec.fetchSpec) };
    } else {
      return (
        "unknown sub spec type. Type = " +
        sub_type +
        ". parsed = " +
        JSON.stringify(parsed_spec)
      );
    }
    if(parse_spec.subSpec.name === null) {
      answer = sub_answer;
    } else {
      answer = { Alias: [parsed_spec.subSpec.name, null, sub_answer] };
    }
  } else {
    return (
      "unknown spec type. Type = " +
      type +
      ". parsed = " +
      JSON.stringify(parsed_spec)
    );
  }

  return JSON.stringify(answer);
}

function remove_prefix(s, p) {
  if (s.startsWith(p)) {
    return s.slice(p.length);
  } else {
    return s;
  }
}

function parse_range(s) {
  const r = new semver.Range(s, { loose: true });
  return r.set.map((conjuncts) =>
    conjuncts.map((comp) => serialize_comparator(comp))
  );
}

function serialize_comparator(c) {
  // console.log(c);
  const op = c.operator;
  const sv = c.semver;

  let key = null;
  if (op == ">") {
    key = "Gt";
  } else if (op == ">=") {
    key = "Gte";
  } else if (op == "") {
    // This is either = or * depending on sv
    if (sv == semver.Comparator.ANY) {
      // *
      key = "Any";
    } else {
      // =
      key = "Eq";
    }
  } else if (op == "<=") {
    key = "Lte";
  } else if (op == "<") {
    key = "Lt";
  } else {
    console.log("unknown comparator op. op = " + op + ". comparator = " + c);
    process.exit(1);
  }

  if (key == "Any") {
    return key;
  } else {
    return { [key]: serialize_semver(sv) };
  }
}

function serialize_semver(v) {
  const prerelease = v.prerelease.map((pre) => {
    if (Number.isInteger(pre)) {
      return { Int: pre };
    } else {
      return { String: pre };
    }
  });

  return {
    major: v.major,
    minor: v.minor,
    bug: v.patch,
    prerelease: prerelease,
    build: v.build,
  };
}

function pidIsRunning(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return false;
  }
}
