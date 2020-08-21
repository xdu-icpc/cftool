# cftool

`cftool` is a command line tool for submitting to [Codeforces][1], and query
the verdict of the submission.

[1]: https://codeforces.com

## Usage

At first you need to create `cftool.json` in your user config directory or
your working directory.  An example is in `example/cftool.json`.

Note that `contest_path` can be a contest, a gym contest, or a group
contest.  And `server` can be `http://codeforces.com`,
`https://codeforces.com`, or `https://codeforc.es`.

Then you can:

* Submit: `cftool -p A -s a.cc`
* Query the verdict of the latest submission in the contest: `cftool -q`.
* Submit and wait until the submission is judged: `cftool -p A -s a.cc -l`.

Use `cftool -h` to see more options.

You may be prompted for password.  `cftool` saves cookies so you won't be
prompted again until the credential expires (in 1 month, just like if you
chose "Remember me for a month" on
[the login page](https://codeforces.com/enter).

You can add `-v` or even `-vv` to see more detail of `cftool`.

### Proxies

Use `http_proxy` environment variable to set proxies for http connections,
or `https_proxy` for https connections.  For example:
`export https_proxy=socks5://example.org:12345`.

## Bugs

`cftool` is not tested in rated contests yet.  Not sure if it will cause
you to be unrated or banned.
