// Installing the app. Chromium hands its install prompt over once, early, whatever page is open: it is kept here for the
// button on «Más». Safari on an iPhone or iPad has no prompt, and installs from its Share menu. Installed, nothing is offered.

type InstallPrompt = Event & {
	prompt: () => Promise<void>;
	userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }>;
};

export const install = $state<{ prompt: InstallPrompt | null; installed: boolean }>({
	prompt: null,
	installed: false
});

if (typeof window !== 'undefined') {
	install.installed =
		matchMedia('(display-mode: standalone)').matches ||
		(navigator as Navigator & { standalone?: boolean }).standalone === true;
	addEventListener('beforeinstallprompt', (event) => {
		event.preventDefault();
		install.prompt = event as InstallPrompt;
	});
	addEventListener('appinstalled', () => {
		install.prompt = null;
		install.installed = true;
	});
}

/** An iPhone or iPad, where a browser adds the app to the home screen from its Share menu. */
export function fromShareMenu(): boolean {
	const apple =
		/iPhone|iPad|iPod/.test(navigator.userAgent) ||
		(navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);
	return apple && !install.installed;
}

/** Shows the browser's own install dialog; a prompt serves once. */
export async function offerInstall() {
	const prompt = install.prompt;
	if (!prompt) return;
	install.prompt = null;
	await prompt.prompt();
	if ((await prompt.userChoice).outcome === 'accepted') install.installed = true;
}
