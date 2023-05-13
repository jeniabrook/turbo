  $ . ${TESTDIR}/../setup.sh non-monorepo yarn
  yarn cache v1.22.17
  success Cleared cache.
  Done in [\.0-9]+m?s\. (re)
  yarn install v1.22.17
  info No lockfile found.
  [1/4] Resolving packages...
  [2/4] Fetching packages...
  [3/4] Linking dependencies...
  [4/4] Building fresh packages...
  success Saved lockfile.
  Done in [\.0-9]+m?s\. (re)

# run twice and make sure it works
  $ yarn turbo build lint --output-logs=none
  yarn run v1.22.17
  \$ (.*)node_modules/.bin/turbo build lint --output-logs=none (re)
  • Running build, lint
  • Remote caching disabled
  
   Tasks:    2 successful, 2 total
  Cached:    0 cached, 2 total
    Time:\s*[\.0-9]+m?s  (re)
  
  Done in [\.0-9]+m?s\. (re)

  $ yarn turbo build lint --output-logs=none
  yarn run v1.22.17
  \$ (.*)node_modules/.bin/turbo build lint --output-logs=none (re)
  • Running build, lint
  • Remote caching disabled
  
   Tasks:    2 successful, 2 total
  Cached:    2 cached, 2 total
    Time:\s*[\.0-9]+m?s >>> FULL TURBO (re)
  
  Done in [\.0-9]+m?s\. (re)

  $ git diff
