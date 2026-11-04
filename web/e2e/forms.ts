// What the forms keep closed, opened as a person opens it.
import type { Page } from '@playwright/test';

/** Opens the fold of the form on the page: «Opciones avanzadas», «Más condiciones». */
export const unfold = (page: Page) => page.locator('form details:not([open]) summary').click();
