package com.shadowsofwar;

import android.app.Activity;
import android.net.Uri;
import android.os.Bundle;

import com.google.android.gms.games.PlayGames;
import com.google.android.gms.games.PlayGamesSdk;
import com.google.android.gms.tasks.Task;

/** Native entry points for Play Games achievements, leaderboards, and events. */
public final class PlayGamesServicesActivity extends Activity {
    private static final int REQUEST_PLAY_GAMES_UI = 4101;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        PlayGamesSdk.initialize(getApplicationContext());
        Uri uri = getIntent().getData();
        String action = uri == null || uri.getPathSegments().isEmpty()
                ? ""
                : uri.getPathSegments().get(0);
        if ("achievements".equals(action)) {
            launchUi(PlayGames.getAchievementsClient(this).getAchievementsIntent());
        } else if ("leaderboards".equals(action)) {
            launchUi(PlayGames.getLeaderboardsClient(this).getAllLeaderboardsIntent());
        } else {
            finish();
        }
    }

    private void launchUi(Task<android.content.Intent> task) {
        task.addOnSuccessListener(this, intent -> startActivityForResult(intent, REQUEST_PLAY_GAMES_UI))
                .addOnFailureListener(this, error -> finish());
    }

}
