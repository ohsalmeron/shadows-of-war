import { chromium } from 'playwright';

(async () => {
  console.log('🚀 Starting Playwright E2E Verification Test (Unified Wiki Database)...');
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  
  const baseUrl = process.env.TEST_URL || 'http://localhost:4321';
  
  console.log(`🔗 Navigating to ${baseUrl}/...`);
  await page.goto(`${baseUrl}/`);
  
  // Verify homepage loads and has the correct title
  const title = await page.title();
  console.log(`📄 Page Title: "${title}"`);
  if (!title.includes('Shadows of War')) {
    throw new Error('Verification Failed: Page title does not contain "Shadows of War"');
  }

  // Check the homepage links straight to the WASM game (no embedded iframe)
  const playHref = await page.locator('a.play-btn').getAttribute('href');
  console.log(`🎮 Play link target: "${playHref}"`);
  if (playHref !== '/play/') {
    throw new Error(`Verification Failed: Play link does not point to "/play/" (got "${playHref}").`);
  }
  console.log('✅ Play link verified.');

  // Click on "Wiki" link
  console.log('👉 Navigating to Unified Wiki Database...');
  await page.click('a[href="/wiki"]');
  
  // Wait for the layout to render
  await page.waitForSelector('.encyclopedia-layout');
  await page.waitForSelector('#wiki-grid');
  console.log('✅ Wiki layout loaded.');

  // Check that the sidebar exists
  const sidebarVisible = await page.isVisible('#filters-sidebar');
  console.log(`📋 Sidebar visible on desktop: ${sidebarVisible}`);
  if (!sidebarVisible) {
    throw new Error('Verification Failed: Sidebar filter panel is not visible on desktop.');
  }

  // Select category "Leader"
  console.log('☑️ Checking "Leader" category filter...');
  await page.click('label:has(input[value="Leader"])');
  await page.waitForTimeout(300);

  // Select region "Americas"
  console.log('☑️ Checking "Americas" region filter...');
  await page.click('label:has(input[value="Americas"])');
  await page.waitForTimeout(300);

  // Check active pills bar is displayed
  const pillsVisible = await page.isVisible('#active-pills-bar');
  if (!pillsVisible) {
    throw new Error('Verification Failed: Active pills bar is not visible after selecting filters.');
  }
  const pillText = await page.innerText('#pills-list');
  console.log(`💊 Active Pill list: "${pillText.replace(/\n/g, ' ')}"`);
  if (!pillText.includes('Leader') || !pillText.includes('Americas')) {
    throw new Error('Verification Failed: Active filter pill list does not contain correct filter labels.');
  }

  // Click Pachacuti card
  const card = page.locator('.leader-card:visible:has-text("Pachacuti")');
  const cardText = await card.innerText();
  console.log(`📇 Pachacuti Card content: "${cardText.replace(/\n/g, ' | ')}"`);
  
  console.log('🖱️ Clicking on Pachacuti card...');
  await card.click();

  // Verify the detail drawer slides open
  await page.waitForSelector('#detail-drawer.open');
  console.log('🔓 Detail drawer is open.');

  const drawerName = await page.innerText('#drawer-name');
  console.log(`👤 Name in drawer: "${drawerName}"`);
  if (drawerName !== 'Pachacuti') {
    throw new Error(`Verification Failed: Drawer shows name "${drawerName}" instead of "Pachacuti"`);
  }

  // Close the drawer
  console.log('📥 Closing the drawer...');
  await page.click('#drawer-close-btn');
  await page.waitForSelector('#detail-drawer:not(.open)');
  console.log('🔒 Drawer closed successfully.');

  // Clear filters
  console.log('✖️ Clearing all filters...');
  await page.click('#clear-filters-btn');
  await page.waitForTimeout(500);

  // Select category "Realm"
  console.log('☑️ Checking "Realm" category filter...');
  await page.click('label:has(input[value="Realm"])');
  await page.waitForTimeout(300);

  // Select region "Europe"
  console.log('☑️ Checking "Europe" region filter...');
  await page.click('label:has(input[value="Europe"])');
  await page.waitForTimeout(300);

  // Click on "Athens" card
  const athensCard = page.locator('.leader-card:visible:has-text("Athens")');
  console.log('🖱️ Clicking on Athens card...');
  await athensCard.click();

  // Verify the detail drawer slides open
  await page.waitForSelector('#detail-drawer.open');
  console.log('🔓 Realms Detail drawer is open.');

  const drawerRealmName = await page.innerText('#drawer-name');
  console.log(`👤 Realm Name in drawer: "${drawerRealmName}"`);
  if (drawerRealmName !== 'Athens') {
    throw new Error(`Verification Failed: Drawer shows realm "${drawerRealmName}" instead of "Athens"`);
  }

  // Close the drawer
  console.log('📥 Closing the drawer...');
  await page.click('#drawer-close-btn');
  await page.waitForSelector('#detail-drawer:not(.open)');

  // ----------------------------------------------------
  // LEGAL PAGES
  // ----------------------------------------------------
  console.log(`🔒 Navigating to ${baseUrl}/privacy...`);
  await page.goto(`${baseUrl}/privacy`);
  const privacyHeading = await page.innerText('h1');
  console.log(`📄 Privacy Heading: "${privacyHeading}"`);
  if (!privacyHeading.includes('Privacy Policy')) {
    throw new Error('Verification Failed: Privacy Policy page header mismatch.');
  }

  console.log(`📜 Navigating to ${baseUrl}/terms...`);
  await page.goto(`${baseUrl}/terms`);
  const termsHeading = await page.innerText('h1');
  console.log(`📄 Terms Heading: "${termsHeading}"`);
  if (!termsHeading.includes('Terms of Service')) {
    throw new Error('Verification Failed: Terms of Service page header mismatch.');
  }

  console.log('🎉 Playwright E2E verification test passed successfully!');
  await browser.close();
  process.exit(0);
})().catch(err => {
  console.error('❌ E2E Verification failed:', err);
  process.exit(1);
});
