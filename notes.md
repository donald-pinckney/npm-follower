# Analysis of Security of the NPM Ecosystem Historically

## Security Vulnerabilities

- How quickly do authors release updates in response to CVEs? {A, M}
  
  Methodology: for each CVE with a patch, we can look at the time between the publish time of the CVE and the publish time of the patched version. We can then look at the distribution of times between CVE publication and fix.
  We can segment this on both the CVE side (severity, vector, etc.) and on the package side (popularity).


- ~~How well are CVEs updated to track fixes or non-fixes to packages? {A, ???}~~ (Federico says they get updated, but do they really?)

## Dependency Structure

- After an update is published, how quickly do downstream packages update to receive it?  {M}
  - Is this different between security updates vs. feature updates?
  
  Methodology: this is probably one of the more complicated ones. In many cases, downstream dependencies *automatically* receive the update,
  if their version constraint allows it. Suppose we are considering an update of package A from version V1 to V2, and we wish to see how often it is adopted.
  First, we get all the packages (B) who's latest version (W1) prior to V2 being published had a dependency on A with constraint C satisfying V1. We then have 2 cases. If 
  C also satisfies V2, then this counts as an *automatic update*. If C does not satisfy V2, then we attempt to find the first version W2 after W1 which does satisfy V2.
  If we find such a version, then we count this as a *manual update*, and we find the time difference between W2 and V2. If we do not find such a version, then we count this as a *non-update*.

  We'd then need to do this for all packages, and then aggregate the results. We can also segment this on upstream package popularity, downstream package popularity, dependency type (prod vs. dev), and update type (security patch vs. other).


## Code changes & semver

Each semver update is one of 4 *update types*: bug (1.1.1 -> 1.1.2), minor (1.1.1 -> 1.2.0), major (1.1.1 -> 2.0.0), or other (betas, and weird crap).

- How frequent are each update type? {M}

  Methodology: for each package, we determine which percentage of its updates are bug, minor, or major (discarding other). We then aggregate this across all packages.

  What constitutes an update? Complications:
  + There is no requirement that versions are increased monotonically. Probably rare, but will happen, so we need to not crash. Example A: 2.0.0, 1.0.0, 0.1.0.
  + Actually common is for updates to be released on multiple "tracks". This needs to be computed right, since I expect it to be common. Example B: 5.5.10, 6.0.0, 6.1.0, 6.2.0, 5.5.11, 6.2.1 (https://www.npmjs.com/package/rxjs?activeTab=versions).

  Example B should result in the updates: 5.5.10 -> 5.5.11 (bug), 6.0.0 -> 6.1.0 (minor), 6.1.0 -> 6.2.0 (minor), 6.2.0 -> 6.2.1 (bug), 5.5.11 -> 6.0.0 (major).

  Proposed method: 
  First, we filter out all versions with prerelease or build tags (e.g. -beta5, etc.).

  For all remaining versions of a package, we first partition into equivalence classes based on semver-compatibility.
  For example, the Example B would be partitioned into {(5.5.10, 5.5.11), (6.0.0, 6.1.0, 6.2.0, 6.2.1)}.
  
  We then assert that each equivalence class is ordered chronologically, if not we report the package as having a malformed history and filter it out.
  I expect that relatively few will be filtered, out, but we should verify.
  
  Now, we build the set of updates. For each equivalence class, we say that there is an update between each version within the class chronologically.
  So for Example B, this would generate the updates 5.5.10 -> 5.5.11 (bug), 6.0.0 -> 6.1.0 (minor), 6.1.0 -> 6.2.0 (minor), 6.2.0 -> 6.2.1 (bug).
  
  Finally, we need to consider the predecessors of the first versions in the classes. Let V be the first chronological version in a class. We note that the
  ordering relation on versions extends to an ordering relation on the equivalency classes. Let Q be the greatest equivalency class which is strictly less than V
  and has at least one member chronologically before V. Let W be the most recent version in Q which occurs chronologically before V. We then say there is an update from W to V.
  For Example B, this would generate the update 5.5.10 -> 6.0.0 (major)


- Among each update type:
  - what files are typically changed? {M, T}
  - how large are the diffs? {M, T}
  - lower-bound on non-breaking changes? {M, T}

  Methodology: For each update, we compute its diff D. From D, we find:
  - how many files are modified (N_F)
  - how many lines are modified/added/removed (N_M, N_A, N_R)
  - which files are modified (S_F)
  - which file extensions are modified (S_E)

  We then normalize these metrics per-package (as was done above), and aggregate across all packages. We also join each update with the update type (determined above),
  and segment by update type. 

  Finally, for each update we can also classify it as definitely non-breaking, or not.
  If only `package.json` is modified, and non-code files (`.md`, etc.) are modified, then it's non-breaking (not quite, for `package.json` we have to check that the dependencies are changed in a non-breaking way, but that's not too hard).
  We could try to do harder things, but likely won't have time.




# Pool of other RQs, not currently being used

- Have install scripts become more or less prevelant over time? Are they only added, or also removed from packages? What are the most frequent commands? {M}
  
  Methodology: let's consider two metrics: a) frequency of packages with at least one version having an install script, and b) frequency of packages where the latest version has an install script.

  We can then compute a) and b) for every month since the start of NPM. 
  Interpretations of a) and b): Metric a) gives an upper bound on how useful install scripts are for developers: __% of packages found it useful to use install scripts at somepoint in their lifecycle. Metric b) says roughly how likely you are to be forced to run an install script if installing a random package at a given point in time.

  Next, let's look at *changes* to install scripts. What % of packages contain in their history these types of changes: 1) add install script, 2) delete install script, 3) change install script? 
  Let's manually examine a few of these in popular packages, and see why developers made those changes.

  Finally, what's the content of install scripts? Let's count command / word occurrences within install scripts. We'll only count a word once for every time it occurrs in any version of a package.

  Note: for all the above, we should ignore beta versions, etc.

- How common are: changing version constraints vs. adding new packages vs. deleting packages  {M}
- Which packages are most important to the ecosystem, and how has this changed over time? (see trivial packages paper, "technical plus factor") {M, D?}
- How have developers used features in JS / TS over time?
- After the release of TypeScript, how rapidly have packages adopted types? Has the rate of adoption sped up or slowed down? {M}
- For packages which adopt types, how do their types evolve over time? {M, T}
- After X (classes, ES modules, async/await, etc.) is introduced to JS, how commonly is it used? {M, T}
- How are typed packages published to NPM? {M, T}
- How many packages (JS?) typecheck with `tsc`? {M, T}
- How many use `eval`? Past / present  {M, T}
- Replication of 2011 article. The one where they scrape JS from top websites and look for `eval`, etc. https://link.springer.com/chapter/10.1007/978-3-642-22655-7_4   {M, T}
- How commonly does autogenerated code occur? {M, T}
- How often are binaries included in packages? Or other types of weird things (GIFs)? {M, T}




# Data that we have

## Metadata {M}
- 2.6 million packages
- 28 million package-version pairs, each one has a url to the repo {R}

## Tarballs {T}
- 28 million tarballs, one for each p-v pair

## GH Advisors (could scrape) {A}
- We could scrape the GH advisory database. It's pretty easy to do.

## Download metrics {D}
- We have download metrics scraped for all packages.

## Repos {R}
- We have repos linked for all package-version pairs





# Related Work

## A Measurement Study of Google Play - https://dl.acm.org/doi/pdf/10.1145/2591971.2592003

- They don't have historical data like we do, they only have data since when they started mining
- They actually reverse engineered the API by disassembling Google Play
- Mined number of free apps vs paid apps over a time span
  - answers: how does the free/paid app ratio change between date x and y?
- Categorized apps, showed ratio of free/paid for each category (one app can belong to only 1 category)
- Showed how star-ratings change based on the app being free/paid and the amount of downloads the app has
- Showed how the number of downloads of an app affects the probability of the app depending on certain libraries (more downloads, more native lib prob)
  - By checking which libraries it depends on, they inferred things like:
    - Which advertisement platform they used (if any)
    - Which social platform is the app connected to (if any)
    - ...
- They showed the amount of duplicate apps in the google play store
- They went through the source code of each app and looked for leaking API secret keys, and found quite a few

## The Evolution of Type Annotations in Python: An Empirical Study - https://www.software-lab.org/publications/fse2022_type_study.pdf

- They use github to mine repos
- They analyzed how many projects are using type annotations in Python over time
- Showed how many people are using which kind of annotation (function argument, return and variable annotations)
- Showed how much of these annotations exist in the projects
  - do they fully annotate everything as if python was a statically typed language?
  - do they occasionally type things here and there?
  - did they have no types before, but then decided to type annotate everything after?
- Out of all the commits in these projects, how many commits where type-related?
- Showed positive relation between type errors in a project and the number of type annotations in the project.
  - (remember: you can ignore type errors in python)


## https://arxiv.org/pdf/1709.04638.pdf

- they mined 169,964 npm packages


## https://ieeexplore.ieee.org/stamp/stamp.jsp?tp=&arnumber=9387131

- this one seems to only analyze 15,000 packages
- but has a lot of information on them


## https://arxiv.org/abs/2210.07482
