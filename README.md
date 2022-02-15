# cftool

`cftool` is a command line tool for submitting to [Codeforces][1], and query
the verdict of the submission.

[1]: https://codeforces.com

## Usage

At first you need to create `cftool.json` in your user config directory or
your working directory.  An example is in `example/cftool.json`.

Note that `contest_path` can be a contest, a gym contest, or a group
contest.  And `server_url` is defaulted to `https://codeforces.com`, normal
users should not override it.

Then you can:

* Submit: `cftool -s a.cc`, or `cftool -p A -s problem-foo.cc`.
* Query the verdict of the latest submission in the contest: `cftool -q`.
* Submit and wait until the submission is judged: `cftool -s a.cc -l`.

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

## Bugs and non-bugs

`cftool` has been tested in many rated contests.  it shouldn't cause you
to be unrated or banned, unless you misused or overused it in some way.

It's known that Codeforces server will throttle the traffic from your IP
if you are making requests too frequently.  In theory it has nothing to do
with `cftool`. But, if you use `cftool` in a script and make some mistake
in your script, the throttle will be more likely to happen.

`cftool` does not support Codeforces basic contest servers (for example,
`https://m2.codeforces.com`) yet.

`cftool` does not support plain HTTP deliberately.  You should use HTTPS
instead.  And, if you override the server URL by any means (for example,
using a third-party reverse proxy server), you should take the security
risks yourself.
