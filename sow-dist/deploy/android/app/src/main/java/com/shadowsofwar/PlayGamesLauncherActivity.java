package com.shadowsofwar;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;
import android.view.Gravity;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;

import com.google.android.gms.games.PlayGames;
import com.google.android.gms.games.PlayGamesSdk;
import com.google.android.gms.games.GamesSignInClient;

import org.json.JSONObject;

import java.io.BufferedReader;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Native Play Games v2 gate before launching the TWA. */
public final class PlayGamesLauncherActivity extends Activity {
    private static final String TAG = "SOW_PGS";
    private final ExecutorService network = Executors.newSingleThreadExecutor();
    private GamesSignInClient signInClient;
    private boolean launching;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Log.i(TAG, "launcher created");
        PlayGamesSdk.initialize(getApplicationContext());
        signInClient = PlayGames.getGamesSignInClient(this);
        showMessage("Connecting to Google Play Games…", false);
        authenticate();
    }

    @Override
    protected void onDestroy() {
        network.shutdownNow();
        super.onDestroy();
    }

    private void authenticate() {
        signInClient.isAuthenticated().addOnCompleteListener(task -> {
            Log.i(TAG, "isAuthenticated success=" + task.isSuccessful());
            if (task.isSuccessful() && task.getResult() != null && task.getResult().isAuthenticated()) {
                Log.i(TAG, "existing Play Games session authenticated");
                requestServerAccess();
            } else {
                Log.i(TAG, "requesting Play Games sign-in");
                signInClient.signIn().addOnCompleteListener(signInTask -> {
                    Log.i(TAG, "signIn success=" + signInTask.isSuccessful());
                    if (signInTask.isSuccessful() && signInTask.getResult() != null && signInTask.getResult().isAuthenticated()) {
                        requestServerAccess();
                    } else {
                        showMessage("Google Play Games is required to play Shadows of War.", true);
                    }
                });
            }
        });
    }

    private void requestServerAccess() {
        String clientId = BuildConfig.PLAY_GAMES_WEB_CLIENT_ID.trim();
        if (clientId.isEmpty()) {
            Log.e(TAG, "server access client ID is empty");
            showMessage("Play Games server access is not configured.", false);
            return;
        }
        PlayGames.getPlayersClient(this).getCurrentPlayer().addOnCompleteListener(playerTask -> {
            if (playerTask.isSuccessful() && playerTask.getResult() != null) {
                showMessage("Welcome, " + playerTask.getResult().getDisplayName() + "!", false);
            }
        });
        signInClient.requestServerSideAccess(clientId, false).addOnCompleteListener(task -> {
            Log.i(TAG, "server access success=" + task.isSuccessful());
            String serverAuthCode = task.isSuccessful() ? task.getResult() : null;
            if (serverAuthCode == null || serverAuthCode.isEmpty()) {
                showMessage("Could not verify your Google Play Games profile.", true);
                return;
            }
            exchangeCode(serverAuthCode);
        });
    }

    private void exchangeCode(String serverAuthCode) {
        showMessage("Verifying your player profile…", false);
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
                        .put("package_name", getPackageName());
                byte[] bytes = body.toString().getBytes(StandardCharsets.UTF_8);
                try (OutputStream output = connection.getOutputStream()) {
                    output.write(bytes);
                }
                int status = connection.getResponseCode();
                InputStream stream = status >= 400 ? connection.getErrorStream() : connection.getInputStream();
                String response = readBody(stream);
                connection.disconnect();
                if (status < 200 || status >= 300) {
                    throw new IllegalStateException("Play Games exchange failed: HTTP " + status);
                }
                String handoff = new JSONObject(response).optString("handoff_token", "");
                if (handoff.isEmpty()) {
                    throw new IllegalStateException("Play Games exchange returned no handoff");
                }
                Log.i(TAG, "server exchange returned handoff");
                runOnUiThread(() -> launchTwa(handoff));
            } catch (Exception error) {
                Log.e(TAG, "server exchange failed", error);
                runOnUiThread(() -> showMessage("Could not connect to Shadows of War. Please retry.", true));
            }
        });
    }

    private void launchTwa(String handoff) {
        if (launching) {
            return;
        }
        launching = true;
        Log.i(TAG, "launching TWA");
        Uri uri = Uri.parse("https://shadowsofwar.io/play/")
                .buildUpon()
                .appendQueryParameter("sow_platform", "android")
                .appendQueryParameter("sow_playgames_handoff", handoff)
                .build();
        startActivity(new Intent(Intent.ACTION_VIEW, uri)
                .setClass(this, com.google.androidbrowserhelper.trusted.LauncherActivity.class)
                .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP));
        finish();
    }

    private void showMessage(String message, boolean retry) {
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setGravity(Gravity.CENTER);
        int padding = (int) (32 * getResources().getDisplayMetrics().density);
        layout.setPadding(padding, padding, padding, padding);

        TextView text = new TextView(this);
        text.setText(message);
        text.setTextSize(18);
        text.setGravity(Gravity.CENTER);
        layout.addView(text, new LinearLayout.LayoutParams(-1, -2));
        if (retry) {
            Button button = new Button(this);
            button.setText("RETRY");
            button.setOnClickListener(view -> authenticate());
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-2, -2);
            params.topMargin = padding;
            layout.addView(button, params);
        }
        setContentView(layout);
    }

    private static String readBody(InputStream stream) throws Exception {
        if (stream == null) {
            return "";
        }
        StringBuilder body = new StringBuilder();
        try (BufferedReader reader = new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8))) {
            String line;
            while ((line = reader.readLine()) != null) {
                body.append(line);
            }
        }
        return body.toString();
    }
}
