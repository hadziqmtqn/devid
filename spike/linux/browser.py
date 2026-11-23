"""Phase 12 L1, question 6: does a browser open https://<site>.test without a certificate error?

    python3 browser.py chrome|firefox <url> <expected text>     exit 0: page loaded with the text

Headless, through Selenium and the runner's chromedriver/geckodriver. Each run uses a fresh
browser profile, as a new user would have; Chrome still reads the user's NSS database
(~/.pki/nssdb), Firefox reads its policies (/etc/firefox/policies/policies.json).
"""

import sys

from selenium import webdriver
from selenium.common.exceptions import WebDriverException

which, url, expected = sys.argv[1], sys.argv[2], sys.argv[3]
if which == "chrome":
    options = webdriver.ChromeOptions()
    options.add_argument("--headless=new")
    driver = webdriver.Chrome(options=options)
else:
    options = webdriver.FirefoxOptions()
    options.add_argument("-headless")
    driver = webdriver.Firefox(options=options)
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
