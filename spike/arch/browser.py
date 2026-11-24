"""Phase 13 A1, question 3: does a browser open https://<site>.test without a certificate error?

    python3 browser.py firefox|chromium <url> <expected text>     exit 0: page loaded with the text

Headless, through Selenium with Arch's own drivers (geckodriver, and the chromedriver that comes
with the chromium package), so Selenium never downloads one. Each run uses a fresh profile, as a new
user's browser would have, so only the system's trust (p11-kit, through Arch's NSS) can make the
page load: nothing is added to a browser database here.
"""

import sys

from selenium import webdriver
from selenium.common.exceptions import WebDriverException

which, url, expected = sys.argv[1], sys.argv[2], sys.argv[3]
if which == "chromium":
    options = webdriver.ChromeOptions()
    options.binary_location = "/usr/bin/chromium"
    options.add_argument("--headless=new")
    options.add_argument("--no-first-run")
    service = webdriver.ChromeService(executable_path="/usr/bin/chromedriver")
    driver = webdriver.Chrome(options=options, service=service)
else:
    options = webdriver.FirefoxOptions()
    options.add_argument("-headless")
    service = webdriver.FirefoxService(executable_path="/usr/bin/geckodriver")
    driver = webdriver.Firefox(options=options, service=service)
driver.set_page_load_timeout(30)
try:
    print(driver.capabilities.get("browserName"), driver.capabilities.get("browserVersion"))
    driver.get(url)
    title, source = driver.title, driver.page_source
    print(f"title: {title!r}")
    if expected in source:
        sys.exit(0)
    print(source[:400])
    sys.exit(1)
except WebDriverException as e:
    print(f"{type(e).__name__}: {str(e).splitlines()[0][:300]}")
    sys.exit(1)
finally:
    driver.quit()
