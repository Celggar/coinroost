import { defineConfig, devices } from '@playwright/test';

// e2e/run.sh starts coinroost-server with this build on a free port and passes its address in BASE_URL.
export default defineConfig({
	testDir: 'e2e',
	// One login per run: a TOTP code is accepted once, so tests cannot log in in parallel.
	workers: 1,
	fullyParallel: false,
	forbidOnly: true,
	retries: 0,
	reporter: [['list']],
	use: {
		baseURL: process.env.BASE_URL,
		trace: 'retain-on-failure'
	},
	// One browser: a second project would need a second login (see above); the phone width is its own test.
	projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }]
});
