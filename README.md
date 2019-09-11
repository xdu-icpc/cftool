# cftool

`cftool` is a command line tool for submitting to Codeforces, and query the
verdict of the submission.

## Usage

At first you need to create `cftool.json` in your user config directory or
your working directory.  An example is in `example/cftool.json`.

Note that `contest_path` can be a contest, a gym contest, or a group
contest.  And `server` can be `http://codeforces.com`,
`https://codeforces.com`, or `https://codeforc.es`.  You should always use
HTTPS if you've set `Enforce https` in Codeforces settings or `cftool` will
malfunction.  It's known using `http://codeforc.es` will cause malfunction.

Then you can:

* Submit: `cftool -p A -s a.cc`
* Query the verdict of the latest submission in the contest: `cftool -q`.
* Submit and wait until the submission is judged: `cftool -p A -s a.cc -l`.

Use `cftool -h` to see more options.

You may be prompted for password.  `cftool` saves cookies so you won't be
prompted again until the session expires (in 24 hours, seemingly).

You can add `-v` or even `-vv` to see more detail of `cftool`.

## Bugs

`cftool is not tested in real contests yet.  Not sure if it will malfunction
or cause you to be banned.
