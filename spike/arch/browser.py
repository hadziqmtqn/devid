"""Phase 13 A1, question 3: does a browser open https://<site>.test without a certificate error?

    python3 browser.py firefox|chromium <url> <expected text> [firefox profile dir]
    exit 0: page loaded with the text

Headless, through Selenium with Arch's own drivers (geckodriver, and the chromedriver that comes
with the chromium package), so Selenium never downloads one. Each run uses a fresh profile, as a new
user's browser would have, so only the system's trust (p11-kit, through Arch's NSS) can make the
page load: nothing is added to a browser database here.

Selenium asks Firefox to accept insecure certificates unless told otherwise (run 5 loaded the site
before the CA was trusted), so both browsers are told not to. On an error the browser's own error
code is printed, from the driver's message or the error page.
"""

import re
import sys
from urllib.parse import unquote

from selenium import webdriver
from selenium.common.exceptions import WebDriverException

CODES = re.compile(r"(?:NET::|net::)ERR_[A-Z_]+|SEC_ERROR_[A-Z_]+|MOZILLA_PKIX_ERROR_[A-Z_]+|SSL_ERROR_[A-Z_]+")

which, url, expected = sys.argv[1], sys.argv[2], sys.argv[3]
profile = sys.argv[4] if len(sys.argv) > 4 and sys.argv[4] else None
if which == "chromium":
    options = webdriver.ChromeOptions()
    options.binary_location = "/usr/bin/chromium"
    options.add_argument("--headless=new")
    options.add_argument("--no-first-run")
    service = webdriver.ChromeService(executable_path="/usr/bin/chromedriver")
else:
    options = webdriver.FirefoxOptions()
    options.add_argument("-headless")
    if profile:
        # geckodriver uses this profile as it is instead of making a fresh one.
        options.add_argument("-profile")
        options.add_argument(profile)
    service = webdriver.FirefoxService(executable_path="/usr/bin/geckodriver")
options.accept_insecure_certs = False
if which == "chromium":
    driver = webdriver.Chrome(options=options, service=service)
else:
    driver = webdriver.Firefox(options=options, service=service)
driver.set_page_load_timeout(30)


def codes(text):
    return set(CODES.findall(unquote(text or "")))


try:
    caps = driver.capabilities
    print(caps.get("browserName"), caps.get("browserVersion"), "acceptInsecureCerts:", caps.get("acceptInsecureCerts"))
    driver.get(url)
    title, source = driver.title, driver.page_source
    print(f"title: {title!r}")
    if expected in source:
        sys.exit(0)
    print("error codes on the page:", sorted(codes(source)))
    print(source[:400])
    sys.exit(1)
except WebDriverException as e:
    message = str(e)
    print(f"{type(e).__name__}: {unquote(message.splitlines()[0])[:500]}")
    found = codes(message)
    try:
        found |= codes(driver.page_source)
    except WebDriverException:
        pass
    print("error codes:", sorted(found))
    sys.exit(1)
finally:
    driver.quit()
