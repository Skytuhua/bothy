"""Google sign-in for apps and agents that run their own OAuth.

Use OAuth clients of your own Google Cloud project; every client of one project sees the same
Drive files. On a machine without a browser:

>>> grant = bothy.auth.device_login("client_tv.json")            # doctest: +SKIP
On a phone or computer, open https://www.google.com/device and enter: ABCD-EFGH
>>> grant.save("token.json")                                       # doctest: +SKIP
>>> tokens = bothy.auth.TokenProvider("client_tv.json", grant)     # doctest: +SKIP
>>> remote = bothy.DriveRemote.folder("robot", tokens)             # doctest: +SKIP

The device flow only grants ``drive.file``, so headless devices use ``DriveRemote.folder``.
Tokens never appear in reprs or exception messages.
"""

import sys
import webbrowser
from typing import Callable, List, Optional

from ._native import Grant, TokenProvider
from ._native import device_login as _native_device_login
from ._native import loopback_login as _native_loopback_login

__all__ = ["Grant", "TokenProvider", "device_login", "loopback_login"]


def _print_code(url: str, code: str) -> None:
    print(f"On a phone or computer, open {url} and enter: {code}", file=sys.stderr, flush=True)


def _open_browser(url: str) -> None:
    print(f"Opening the browser to sign in. If it does not open, visit:\n\n  {url}\n",
          file=sys.stderr, flush=True)
    webbrowser.open(url)


def device_login(client_file: str, show: Callable[[str, str], None] = _print_code) -> Grant:
    """Device sign-in: ``show(url, code)`` tells the user what to enter where, then this waits
    until they approve. Needs a "TVs and Limited Input devices" client."""
    return _native_device_login(client_file, show)


def loopback_login(client_file: str, show: Callable[[str], None] = _open_browser,
                   scopes: Optional[List[str]] = None) -> Grant:
    """Browser sign-in with a ``127.0.0.1`` redirect and PKCE. Needs a "Desktop app" client."""
    return _native_loopback_login(client_file, show, scopes)

