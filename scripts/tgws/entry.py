"""PyInstaller entry point for the headless Telegram proxy.

PyInstaller cannot bundle `proxy/tg_ws_proxy.py` directly: that module uses
relative imports (`from .config import ...`) and only works when imported as a
package module. This launcher gives the frozen binary the same entry point as
the upstream `tg-ws-proxy` console script.
"""

from proxy.tg_ws_proxy import main

if __name__ == "__main__":
    main()
