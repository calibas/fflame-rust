"""Simple test to verify browser launches and loads page"""

import subprocess
import time
from selenium import webdriver
from selenium.webdriver.chrome.options import Options

# Start HTTP server
print('Starting HTTP server...')
server = subprocess.Popen(
    ['python', '-m', 'http.server', '8080'],
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL,
)
time.sleep(1)

try:
    # Launch browser (visible, not headless)
    print('Launching Chrome...')
    options = Options()
    # No headless - window should be visible
    options.add_argument('--window-size=1920,1080')

    driver = webdriver.Chrome(options=options)
    print('Chrome launched!')

    # Navigate to test page
    print('Navigating to http://localhost:8080/tests/visual/wasm/test.html')
    driver.get('http://localhost:8080/tests/visual/wasm/test.html')

    print('Page loaded! Browser should be visible now.')
    print('Press Enter to close...')
    input()

    driver.quit()

finally:
    server.terminate()
    server.wait()
    print('Done!')
