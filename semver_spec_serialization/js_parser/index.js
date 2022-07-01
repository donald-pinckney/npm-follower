const npa = require('npm-package-arg');
const semver = require('semver');

if (process.argv.length != 3) {
    console.log("usage: [spec to parse]")
    process.exit(1);
}

const raw_spec = process.argv[2].trim();

let parsed_spec = null;
try {
    parsed_spec = npa.resolve('foo', raw_spec);
} catch(err) {
    console.log(JSON.stringify({'Err': err.code + ": " + err.message}))
    process.exit(0);
}
const type = parsed_spec.type;


let answer = null;
if(type == 'git') {
    answer = {'Git': parsed_spec.saveSpec};
} else if(type == 'version' || type == 'range') {
    answer = {'Range': parse_range(parsed_spec.fetchSpec)}
} else if(type == 'tag') {
    answer = {'Tag': parsed_spec.fetchSpec};
} else if(type == 'file') {
    answer = {'File': remove_prefix(parsed_spec.rawSpec, 'file:')};
} else if(type == 'directory') {
    answer = {'Directory': remove_prefix(parsed_spec.rawSpec, 'file:')};
} else if(type == 'remote') {
    answer = {'Remote': parsed_spec.rawSpec};
} else if(type == 'alias') {
    const sub_type = parsed_spec.subSpec.type;
    let sub_answer = null;
    if(sub_type == 'tag') {
        sub_answer = {'Tag': parsed_spec.subSpec.fetchSpec}
    } else if(sub_type == 'version' || sub_type == 'range') {
        sub_answer = {'Range': parse_range(parsed_spec.subSpec.fetchSpec)}
    } else {
        console.log("unknown sub spec type. Type = " + sub_type + ". parsed = " + JSON.stringify(parsed_spec));
        process.exit(1);
    }
    answer = {'Alias': [parsed_spec.subSpec.name, null, sub_answer]}
} else {
    console.log("unknown spec type. Type = " + type + ". parsed = " + JSON.stringify(parsed_spec));
    process.exit(1);
}


// console.log(parsed_spec)
console.log(JSON.stringify({'Ok': answer}))


function remove_prefix(s, p) {
    if(s.startsWith(p)) {
        return s.slice(p.length)
    } else {
        return s
    }
}

function parse_range(s) {
    const r = new semver.Range(s, {loose: true});
    return r.set.map(conjuncts => conjuncts.map(comp => serialize_comparator(comp)));
}

function serialize_comparator(c) {
    // console.log(c);
    const op = c.operator;
    const sv = c.semver;

    let key = null;
    if(op == '>') {
        key = 'Gt';
    } else if(op == '>=') {
        key = 'Gte';
    } else if(op == '') {
        // This is either = or * depending on sv
        if(sv == semver.Comparator.ANY) {
            // *
            key = 'Any';
        } else {
            // =
            key = 'Eq';
        }
    } else if(op == '<=') {
        key = 'Lte';
    } else if(op == '<') {
        key = 'Lt';
    } else {
        console.log("unknown comparator op. op = " + op + ". comparator = " + c);
        process.exit(1);
    }

    if(key == 'Any') {
        return key
    } else {
        return {[key]: serialize_semver(sv)}
    }
}

function serialize_semver(v) {
    const prerelease = v.prerelease.map(pre => {
        if(Number.isInteger(pre)) {
            return {'Int': pre}
        } else {
            return {'String': pre}
        }
    });

    return {
        major: v.major,
        minor: v.minor,
        bug: v.patch,
        prerelease: prerelease,
        build: v.build
    }
}
