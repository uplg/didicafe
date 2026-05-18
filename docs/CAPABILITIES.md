# DidiCafe — Capabilities

A complete overview of what DidiCafe does, written for end-users who just want to understand the system — no technical knowledge required.

---

## What Is DidiCafe?

DidiCafe is a WiFi access management system for cafes, restaurants, co-working spaces, and any venue that wants to offer paid, time-limited internet access to customers.

Customers connect to the WiFi, enter a token code they received after paying, and get internet access for the duration they purchased. When time runs out, access stops automatically.

---

## For Customers

### Connecting to the WiFi

1. **Find the network** — Look for the WiFi network named after the venue (e.g., "DidiCafe") on your phone, laptop, or tablet.
2. **Captive portal opens automatically** — Your device detects that this is a login-required network and shows a splash page. If it doesn't open automatically, open your browser and visit any website — you'll be redirected.

> ![Captive portal splash page — customer enters their token](../docs/screenshots/portal-splash.png)
> *Screenshot: The portal page where customers enter their access token*

3. **Enter your token** — Type the code you received after paying (format: `DIDI-XXXX-XXXX`) and press **Connect**.
4. **You're online** — A confirmation page appears. You can now browse freely until your time runs out.

> ![Connection success page](../docs/screenshots/portal-success.png)
> *Screenshot: Confirmation page after a successful connection*

### Checking Your Remaining Time

At any point, visit the **Status** page from the portal to see how much time you have left. A live countdown shows minutes remaining.

> ![Session status page with countdown timer](../docs/screenshots/portal-status.png)
> *Screenshot: The status page showing remaining session time*

### When Your Session Expires

When your purchased time runs out, all your web requests are redirected back to the portal with a message that your session has expired. To continue browsing, purchase a new token.

> ![Session expired page](../docs/screenshots/portal-expired.png)
> *Screenshot: Page shown when a session has expired*

### Available Plans

The portal displays all available plans (duration and price) so you know what's offered before purchasing.

> ![Plans listing page](../docs/screenshots/portal-plans.png)
> *Screenshot: Available access plans with durations and prices*

---

## For Staff / Venue Managers

### Admin Dashboard

Log in to the admin panel to manage everything. The dashboard shows at a glance:

- **Active sessions** — How many customers are currently connected
- **Tokens sold today** — Number of tokens redeemed
- **Revenue today** — Total earnings in your local currency
- **7-day trend chart** — Visual overview of tokens sold and revenue over the past week

> ![Admin dashboard with stats and 7-day chart](../docs/screenshots/admin-dashboard.png)
> *Screenshot: The admin dashboard with key metrics and weekly chart*

### Managing Sessions

See every currently connected customer: their token code, device identifier, IP address, and how much time they have remaining. You can **disconnect** any session instantly — the customer loses access immediately.

> ![Active sessions list with disconnect buttons](../docs/screenshots/admin-sessions.png)
> *Screenshot: Active sessions table with real-time remaining time*

### Creating Access Plans

Define the plans you want to offer. Each plan has:

- **Name** — e.g., "1 Hour", "2 Hours", "Half Day"
- **Duration** — in minutes
- **Price** — in your local currency

You can activate or deactivate plans at any time without deleting them.

> ![Plans management — create, edit, activate/deactivate](../docs/screenshots/admin-plans.png)
> *Screenshot: Plan creation and management interface*

### Generating Tokens

Generate one or many tokens for any plan:

1. Choose a plan from the dropdown
2. Enter the quantity (1 to 100 at once)
3. Optionally give the batch a name (e.g., "Morning batch")
4. Click **Generate**

Tokens are displayed immediately. Copy them individually or write them down. Each token can only be used once.

> ![Token generation form and results](../docs/screenshots/admin-tokens.png)
> *Screenshot: Token generation with batch creation*

### Token Inventory

View all tokens — unused, active, expired, or revoked — with status filters and pagination. Copy any token code with one click. Revoke tokens that haven't been used yet if needed.

> ![Full token list with status filters](../docs/screenshots/admin-token-list.png)
> *Screenshot: Token inventory with status filtering*

### Branding & Settings

Customize the customer-facing portal to match your venue:

- **Venue name** — Displayed on the portal page
- **Welcome message** — A greeting in your language(s)
- **Theme color** — Match your brand colors
- **Contact information** — Name, phone number, opening hours — shown on the portal for customer support

> ![Settings page with branding customization](../docs/screenshots/admin-settings.png)
> *Screenshot: Branding and contact settings*

### Audit Log

Every action in the admin panel is recorded: logins, token generation, session disconnections, plan changes. The audit log helps you track who did what and when.

> ![Audit log table with timestamps and actions](../docs/screenshots/admin-audit.png)
> *Screenshot: Audit trail of all admin actions*

---

## Key Features at a Glance

| Feature | Description |
|---------|-------------|
| **Automatic captive portal** | Customers are redirected to the login page automatically on all devices (iOS, Android, Windows, Mac, Linux) |
| **Time-limited access** | Each token grants a specific duration. Access stops automatically when time expires |
| **Multiple plans** | Offer different durations at different prices (e.g., 1h, 2h, 3h) |
| **Real-time monitoring** | See who's connected and how much time they have left |
| **Instant disconnect** | Cut off any session with one click |
| **Revenue tracking** | Daily revenue and 7-day trend chart |
| **Audit trail** | Every admin action is logged |
| **Custom branding** | Match the portal to your venue's look and feel |
| **Multi-language ready** | Portal supports multiple languages for customers |
| **Privacy page** | Built-in privacy policy page for transparency |
| **Session recovery** | If the device reboots, active sessions are restored automatically — customers don't lose their remaining time |
| **Brute-force protection** | Too many wrong token attempts temporarily blocks the device |

---

## What Customers Don't Need to Know

DidiCafe handles all the technical complexity behind the scenes:

- **No app to install** — Works in any browser
- **No account to create** — Just enter the token
- **No personal data required** — The system only tracks the device and session time
- **Works on any device** — Phones, laptops, tablets, gaming consoles
