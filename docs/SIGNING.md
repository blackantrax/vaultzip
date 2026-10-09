# Code signing setup

Releases are built so that signing switches on with one repository setting. Until you finish this guide, releases still publish normally but unsigned, and Windows SmartScreen will show an "unknown publisher" warning on the installer.

When signing is enabled, the release workflow signs vaultzip-gui.exe and vaultzip.exe, builds the installer from the signed files, signs the installer, then checks every signature (valid and timestamped) before anything is published. A bad signature stops the release.

## Before you start

The workflow uses Azure Artifact Signing (formerly Trusted Signing). Check these points first, because Microsoft changes them over time and the Azure portal shows the current rules:

- Public Trust certificates are available to organizations in the United States, Canada, the European Union and the United Kingdom, and to individual developers in the United States and Canada. Microsoft's quickstart now lists additional countries for organizations.
- Microsoft's documentation states that organizations need a verifiable tax history of three years or more. If your company is younger, validation can fail with no exception.
- Individual validation uses your personal legal name, so the publisher shown to users would be your name instead of the company name.
- The Azure subscription must be a paid one. Free trial subscriptions are not accepted.

If you are not eligible, use an OV or EV code signing certificate from a certificate authority instead. EV certificates give the fastest SmartScreen reputation but need a hardware token or cloud key service, which makes GitHub Actions setup more involved.

## Step 1: Azure

1. Create an Artifact Signing account in the Azure portal. Note the region you choose, since the endpoint depends on it.
2. Under the account, create an identity validation (Public) with the legal company name and business identifier. Validation is done only in the Azure portal and can take days.
3. After validation succeeds, create a Public Trust certificate profile.
4. Create an app registration (Microsoft Entra ID) for GitHub. On it, add a federated credential:
   - Scenario: GitHub Actions deploying Azure resources
   - Organization and repository: your GitHub owner and the vaultzip repository
   - Entity type: Environment
   - Environment name: release
5. Give that app the role Artifact Signing Certificate Profile Signer on the certificate profile (or on the account).

No password or key is stored in GitHub. The workflow proves its identity with a short-lived token.

## Step 2: GitHub

1. In the repository, open Settings, then Environments, and create an environment named release. Add yourself as a required reviewer. Every release then waits for your approval before it can sign anything.
2. Under Settings, Secrets and variables, Actions, add these repository secrets, taken from the app registration and your subscription:
   - AZURE_CLIENT_ID
   - AZURE_TENANT_ID
   - AZURE_SUBSCRIPTION_ID
3. On the Variables tab add:
   - SIGNING_ENDPOINT: the endpoint for your account's region, shown in the Azure portal (it looks like https://eus.codesigning.azure.net/)
   - SIGNING_ACCOUNT: the Artifact Signing account name
   - SIGNING_PROFILE: the certificate profile name
   - SIGNING_ENABLED: true

## Step 3: Try it

Push a tag such as v0.1.0-rc1 and approve the release when GitHub asks. In the Windows job, the Verify signatures step prints the publisher name found on each file. Download the installer, right-click it, open Properties, and check the Digital Signatures tab.

## Notes

- Signatures are timestamped, so they remain valid after the short-lived signing certificate expires.
- The uninstaller that the installer writes to the user's machine is generated at install time and is not signed. Signing it requires an Inno Setup signing command configured for the build, which is a possible later improvement.
- A valid signature shows your verified publisher name. SmartScreen reputation is decided separately, so a very new file can still show a warning at first. Check the behavior on your first real release before promoting the download.
- Pin the actions in the workflows to full commit hashes once the release process is stable. Dependabot is configured to propose updates weekly. Signing workflows are a high-value target, so keep the release environment protected with required reviewers.
