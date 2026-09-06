/**
 * World of Unreal Universal Identity & Cross-Game SDK
 * Shadows of War Web Integration
 * (C) 2026 World of Unreal. MIT License.
 */

(function (global) {
  const ID_SERVER_URL = 'https://id.worldofunreal.com';
  const AUTH_HUB_CALLBACK_URL = 'https://worldofunreal.com/auth/callback';

  class WouAuthClient {
    constructor(defaultContext = 'shadows_of_war') {
      this.defaultContext = defaultContext;
      this.sessionToken = null;
      this.user = null;
      if (typeof window !== 'undefined') {
        this.initSession();
      }
    }

    initSession() {
      // 1. Check if returning from cross-domain SSO Hub with token in URL
      const urlParams = new URLSearchParams(window.location.search);
      const tokenFromUrl = urlParams.get('session_token');
      const accParam = urlParams.get('account');

      if (tokenFromUrl && accParam) {
        try {
          const account = JSON.parse(decodeURIComponent(accParam));
          this.setSession(tokenFromUrl, account);

          // Clean query parameters from address bar cleanly without page refresh
          urlParams.delete('session_token');
          urlParams.delete('account');
          const cleanSearch = urlParams.toString();
          const newUrl = window.location.pathname + (cleanSearch ? `?${cleanSearch}` : '') + window.location.hash;
          window.history.replaceState({}, document.title, newUrl);
          return;
        } catch (err) {
          console.error('Failed to parse returning SSO account payload:', err);
        }
      }

      // 2. Hydrate from localStorage
      const savedToken = localStorage.getItem('wou_session_token');
      const savedUser = localStorage.getItem('wou_user_data');

      if (savedToken && savedUser) {
        try {
          this.sessionToken = savedToken;
          this.user = JSON.parse(savedUser);
        } catch (err) {
          console.error('Failed to parse local stored session:', err);
          this.logout();
        }
      }
      return this.user;
    }

    loadSession() {
      if (typeof window !== 'undefined') {
        this.initSession();
      }
      return this.user;
    }

    setSession(token, account) {
      this.sessionToken = token;
      this.user = account;
      if (typeof window !== 'undefined') {
        localStorage.setItem('wou_session_token', token);
        localStorage.setItem('wou_user_data', JSON.stringify(account));
        window.dispatchEvent(new CustomEvent('wou:auth-state-change', {
          detail: { authenticated: true, user: account, token },
        }));
      }
    }

    logout() {
      this.sessionToken = null;
      this.user = null;
      if (typeof window !== 'undefined') {
        localStorage.removeItem('wou_session_token');
        localStorage.removeItem('wou_user_data');
        window.dispatchEvent(new CustomEvent('wou:auth-state-change', {
          detail: { authenticated: false, user: null, token: null },
        }));
      }
    }

    getUser() {
      return this.user;
    }

    getSessionToken() {
      return this.sessionToken;
    }

    isAuthenticated() {
      return !!this.sessionToken && !!this.user;
    }

    openModal() {
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('wou:open-auth-modal'));
        const modal = document.getElementById('wou-auth-modal');
        if (modal) modal.classList.remove('hidden');
      }
    }

    closeModal() {
      if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('wou:close-auth-modal'));
        const modal = document.getElementById('wou-auth-modal');
        if (modal) modal.classList.add('hidden');
      }
    }

    async startAnonymous(context, displayName) {
      const res = await fetch(`${ID_SERVER_URL}/api/v1/auth/anonymous`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          context: context || this.defaultContext,
          display_name: displayName,
        }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Failed to start anonymous session.');
      this.setSession(data.session_token, data.account);
      this.closeModal();
      return data;
    }

    async requestOtp(email, newsletterOptIn = false, context) {
      const res = await fetch(`${ID_SERVER_URL}/api/v1/auth/otp/request`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email,
          account_id: this.user?.id || null,
          context: context || this.defaultContext,
          newsletter_opt_in: newsletterOptIn,
        }),
      });
      const data = await res.json();
      if (!res.ok) {
        const error = new Error(data.error || 'Failed to dispatch verification code.');
        error.data = data;
        throw error;
      }
      return data;
    }

    describeOtpError(error) {
      const retry = Number(error?.data?.retry_after_seconds);
      if (Number.isFinite(retry) && retry > 0) {
        const minutes = Math.floor(retry / 60);
        const seconds = retry % 60;
        const wait = minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`;
        return { message: `Too many codes requested. Wait ${wait} before trying again.`, retryAfterSeconds: Math.ceil(retry) };
      }
      return { message: error?.message || 'Failed to dispatch verification code.' };
    }

    async verifyOtp(email, code, context) {
      const res = await fetch(`${ID_SERVER_URL}/api/v1/auth/otp/verify`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email,
          code,
          account_id: this.user?.id || null,
          context: context || this.defaultContext,
        }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Invalid or expired verification code.');
      this.setSession(data.session_token, data.account);
      this.closeModal();
      return data;
    }

    loginWithOAuth(provider) {
      const returnTo = typeof window !== 'undefined' ? window.location.href : '';
      const accountId = this.user?.id || '';

      if (typeof window !== 'undefined') {
        sessionStorage.setItem('wou_oauth_provider', provider);
      }

      const stateObj = {
        returnTo,
        accountId,
        provider,
      };

      let statePayload = '';
      try {
        statePayload = btoa(unescape(encodeURIComponent(JSON.stringify(stateObj))))
          .replace(/\+/g, '-')
          .replace(/\//g, '_')
          .replace(/=+$/, '');
      } catch (e) {
        statePayload = encodeURIComponent(JSON.stringify(stateObj));
      }

      const targetUrl = `${ID_SERVER_URL}/api/v1/auth/oauth/login/${provider}?redirect_uri=${encodeURIComponent(AUTH_HUB_CALLBACK_URL)}&state=${encodeURIComponent(statePayload)}`;
      if (typeof window !== 'undefined') {
        window.location.href = targetUrl;
      }
    }

    loginWithSocial(provider) {
      return this.loginWithOAuth(provider);
    }

    async loginWithEthereum() {
      const ethereum = window?.ethereum;
      if (!ethereum) throw new Error('MetaMask / EVM wallet not detected. Please install MetaMask.');
      const accounts = await ethereum.request({ method: 'eth_requestAccounts' });
      const publicAddress = accounts[0];

      const challengeRes = await fetch(`${ID_SERVER_URL}/api/v1/auth/web3/challenge`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ chain: 'ethereum', public_address: publicAddress }),
      });
      const challengeData = await challengeRes.json();
      if (!challengeRes.ok) throw new Error(challengeData.error || 'Failed to initiate Web3 challenge.');

      const signature = await ethereum.request({
        method: 'personal_sign',
        params: [challengeData.message, publicAddress],
      });

      const verifyRes = await fetch(`${ID_SERVER_URL}/api/v1/auth/web3/verify`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          chain: 'ethereum',
          public_address: publicAddress,
          signature,
          message: challengeData.message,
          account_id: this.user?.id || null,
          context: this.defaultContext,
        }),
      });
      const data = await verifyRes.json();
      if (!verifyRes.ok) throw new Error(data.error || 'Ethereum signature verification failed.');
      this.setSession(data.session_token, data.account);
      this.closeModal();
      return data;
    }

    async loginWithEvm() {
      return this.loginWithEthereum();
    }

    async loginWithSolana() {
      const phantom = window?.phantom?.solana || window?.solana;
      if (!phantom || !phantom.isPhantom) throw new Error('Phantom wallet not detected. Please install Phantom.');
      const connectResp = await phantom.connect();
      const publicAddress = connectResp.publicKey.toString();

      const challengeRes = await fetch(`${ID_SERVER_URL}/api/v1/auth/web3/challenge`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ chain: 'solana', public_address: publicAddress }),
      });
      const challengeData = await challengeRes.json();
      if (!challengeRes.ok) throw new Error(challengeData.error || 'Failed to initiate Solana challenge.');

      const messageBytes = new TextEncoder().encode(challengeData.message);
      const signedData = await phantom.signMessage(messageBytes, 'utf8');

      let signatureHex = '';
      if (signedData.signature) {
        const sigArr = Array.from(new Uint8Array(signedData.signature));
        signatureHex = '0x' + sigArr.map((b) => b.toString(16).padStart(2, '0')).join('');
      }

      const verifyRes = await fetch(`${ID_SERVER_URL}/api/v1/auth/web3/verify`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          chain: 'solana',
          public_address: publicAddress,
          signature: signatureHex,
          message: challengeData.message,
          account_id: this.user?.id || null,
          context: this.defaultContext,
        }),
      });
      const data = await verifyRes.json();
      if (!verifyRes.ok) throw new Error(data.error || 'Solana signature verification failed.');
      this.setSession(data.session_token, data.account);
      this.closeModal();
      return data;
    }

    async loginWithPasskey() {
      if (typeof window === 'undefined' || !window.PublicKeyCredential) {
        throw new Error('WebAuthn / Passkeys are not supported on this browser.');
      }
      const challengeRes = await fetch(`${ID_SERVER_URL}/api/v1/auth/web3/challenge`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ chain: 'passkey', public_address: this.user?.username || 'anonymous' }),
      });
      const challengeData = await challengeRes.json();
      if (!challengeRes.ok) throw new Error(challengeData.error || 'Failed to initiate Passkey challenge.');

      const challengeBuffer = Uint8Array.from(atob(challengeData.message.slice(0, 32)), c => c.charCodeAt(0));
      const credential = (await navigator.credentials.get({
        publicKey: {
          challenge: challengeBuffer,
          timeout: 60000,
          userVerification: 'preferred',
        },
      }));

      if (!credential) throw new Error('Passkey authentication cancelled or failed.');

      const verifyRes = await fetch(`${ID_SERVER_URL}/api/v1/auth/web3/verify`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          chain: 'passkey',
          public_address: credential.id,
          signature: 'PASSKEY_ASSERTION_VERIFIED',
          message: challengeData.message,
          account_id: this.user?.id || null,
          context: this.defaultContext,
        }),
      });
      const data = await verifyRes.json();
      if (!verifyRes.ok) throw new Error(data.error || 'Passkey verification failed.');
      this.setSession(data.session_token, data.account);
      this.closeModal();
      return data;
    }

    async searchPlayers(query, limit = 10) {
      const clean = query.trim();
      if (!clean) return [];
      try {
        const res = await fetch(`${ID_SERVER_URL}/api/v1/user/search?q=${encodeURIComponent(clean)}&limit=${limit}`);
        if (!res.ok) return [];
        return await res.json();
      } catch {
        return [];
      }
    }

    async getClanDetails(tag) {
      try {
        const res = await fetch(`${ID_SERVER_URL}/api/v1/clans/${encodeURIComponent(tag)}`);
        if (!res.ok) return null;
        return await res.json();
      } catch {
        return null;
      }
    }

    async getClan(tag) {
      return this.getClanDetails(tag);
    }
  }

  const wouAuth = new WouAuthClient();
  global.ID_SERVER_URL = ID_SERVER_URL;
  global.AUTH_HUB_CALLBACK_URL = AUTH_HUB_CALLBACK_URL;
  global.WouAuthClient = WouAuthClient;
  global.wouAuth = wouAuth;
})(typeof window !== 'undefined' ? window : globalThis);
