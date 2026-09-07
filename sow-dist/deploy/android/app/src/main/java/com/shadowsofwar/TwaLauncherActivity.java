package com.shadowsofwar;

import android.net.Uri;
import android.os.Bundle;
import android.util.Log;

import com.google.androidbrowserhelper.trusted.LauncherActivity;
import com.google.android.gms.games.GamesSignInClient;
import com.google.android.gms.games.PlayGames;
import com.google.android.gms.games.PlayGamesSdk;

import org.json.JSONObject;

import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Opens the TWA and performs a best-effort Play Games identity handoff. */
public final class TwaLauncherActivity extends LauncherActivity {
    private static final String TAG = "SOW_PGS";
    private final ExecutorService network = Executors.newSingleThreadExecutor();
    private GamesSignInClient signInClient;
    private boolean requestInFlight;
    private String rendezvousId;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Log.i(TAG, "TWA launched; starting Play Games in parallel");
        PlayGamesSdk.initialize(getApplicationContext());
        signInClient = PlayGames.getGamesSignInClient(this);
        checkAuthentication();
    }

    @Override
    protected Uri getLaunchingUrl() {
        rendezvousId = UUID.randomUUID().toString().replace("-", "");
        return Uri.parse("https://shadowsofwar.io/play/")
                .buildUpon()
                .appendQueryParameter("sow_platform", "android")
                .appendQueryParameter("sow_playgames_rendezvous", rendezvousId)
                .build();
    }

    @Override
    protected void onDestroy() {
        network.shutdownNow();
        super.onDestroy();
    }

    private void checkAuthentication() {
        if (requestInFlight || signInClient == null) {
            return;
        }
        requestInFlight = true;
        signInClient.isAuthenticated().addOnCompleteListener(task -> {
            Log.i(TAG, "isAuthenticated success=" + task.isSuccessful());
            if (task.isSuccessful() && task.getResult() != null && task.getResult().isAuthenticated()) {
                Log.i(TAG, "automatic Play Games session authenticated");
                requestServerAccess();
            } else {
                requestInFlight = false;
                Log.i(TAG, "automatic Play Games authentication unavailable; continuing anonymously");
            }
        });
    }

    private void requestServerAccess() {
        String clientId = BuildConfig.PLAY_GAMES_WEB_CLIENT_ID.trim();
        if (clientId.isEmpty()) {
            requestInFlight = false;
            Log.e(TAG, "server access client ID is empty");
            return;
        }
        signInClient.requestServerSideAccess(clientId, false).addOnCompleteListener(task -> {
            Log.i(TAG, "server access success=" + task.isSuccessful());
            String serverAuthCode = task.isSuccessful() ? task.getResult() : null;
            if (serverAuthCode == null || serverAuthCode.isEmpty()) {
                requestInFlight = false;
                Log.w(TAG, "automatic Play Games server access unavailable");
                return;
            }
            exchangeCode(serverAuthCode);
        });
    }

    private void exchangeCode(String serverAuthCode) {
        Log.i(TAG, "exchanging server auth code");
        network.execute(() -> {
            try {
                URL url = new URL(BuildConfig.PLAY_GAMES_AUTH_URL + "/auth/playgames/exchange");
                HttpURLConnection connection = (HttpURLConnection) url.openConnection();
                connection.setRequestMethod("POST");
                connection.setConnectTimeout(5000);
                connection.setReadTimeout(5000);
                connection.setDoOutput(true);
                connection.setRequestProperty("Content-Type", "application/json");
                JSONObject body = new JSONObject()
                        .put("server_auth_code", serverAuthCode)
                        .put("package_name", getPackageName())
                        .put("rendezvous_id", rendezvousId);
                byte[] bytes = body.toString().getBytes(StandardCharsets.UTF_8);
                try (OutputStream output = connection.getOutputStream()) {
                    output.write(bytes);
                }
                int status = connection.getResponseCode();
                if (status < 200 || status >= 300) {
                    Log.i(TAG, "Play Games handoff unavailable; continuing anonymously (HTTP " + status + ")");
                } else {
                    Log.i(TAG, "Play Games rendezvous is ready");
                }
                connection.disconnect();
            } catch (Exception error) {
                Log.i(TAG, "Play Games handoff unavailable; continuing anonymously");
            }
            runOnUiThread(() -> requestInFlight = false);
        });
    }
}
