package com.shadowsofwar;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;
import android.widget.TextView;

import com.revenuecat.purchases.CustomerInfo;
import com.revenuecat.purchases.Offering;
import com.revenuecat.purchases.Offerings;
import com.revenuecat.purchases.Package;
import com.revenuecat.purchases.Purchases;
import com.revenuecat.purchases.PurchasesConfiguration;
import com.revenuecat.purchases.PurchasesError;
import com.revenuecat.purchases.PurchaseParams;
import com.revenuecat.purchases.interfaces.PurchaseCallback;
import com.revenuecat.purchases.interfaces.ReceiveCustomerInfoCallback;
import com.revenuecat.purchases.interfaces.ReceiveOfferingsCallback;
import com.revenuecat.purchases.models.StoreTransaction;

import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;

/** Native Google Play checkout bridge for the web game shell. */
public final class PurchaseActivity extends Activity {
    private static final String TAG = "SOW Purchases";
    private static final String RETURN_URL = "https://shadowsofwar.io/play/?sow_platform=android";
    private static final Set<String> GEM_PRODUCTS = new HashSet<>(Arrays.asList(
            "sow_gems_500",
            "sow_gems_1200",
            "sow_gems_2600"
    ));

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        handleIntent(getIntent());
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        handleIntent(intent);
    }

    private void handleIntent(Intent intent) {
        Uri uri = intent == null ? null : intent.getData();
        if (uri == null || !"sow".equals(uri.getScheme())) {
            fail("Invalid purchase request");
            return;
        }
        String appUserId = uri.getQueryParameter("app_user_id");
        if (!isPurchaseUserId(appUserId)) {
            fail("Invalid player account");
            return;
        }
        if ("restore".equals(uri.getHost())) {
            restore(appUserId);
            return;
        }
        String productId = uri.getQueryParameter("product_id");
        if (!"purchase".equals(uri.getHost()) || !GEM_PRODUCTS.contains(productId)) {
            fail("Invalid store product");
            return;
        }
        if (!configure(appUserId)) {
            return;
        }
        setMessage("Opening Google Play…");
        Purchases.getSharedInstance().getOfferings(new ReceiveOfferingsCallback() {
            @Override
            public void onReceived(Offerings offerings) {
                Offering current = offerings.getCurrent();
                if (current == null) {
                    fail("Store is temporarily unavailable");
                    return;
                }
                Package productPackage = null;
                for (Package candidate : current.getAvailablePackages()) {
                    if (productId.equals(candidate.getProduct().getId())) {
                        productPackage = candidate;
                        break;
                    }
                }
                if (productPackage == null) {
                    fail("Product is not available");
                    return;
                }
                Purchases.getSharedInstance().purchase(
                        new PurchaseParams.Builder(PurchaseActivity.this, productPackage).build(),
                        new PurchaseCallback() {
                            @Override
                            public void onCompleted(StoreTransaction transaction, CustomerInfo customerInfo) {
                                returnToGame("success", productId);
                            }

                            @Override
                            public void onError(PurchasesError error, boolean userCancelled) {
                                returnToGame(userCancelled ? "cancelled" : "error", productId);
                            }
                        });
            }

            @Override
            public void onError(PurchasesError error) {
                fail("Store is temporarily unavailable");
            }
        });
    }

    private void restore(String appUserId) {
        if (!configure(appUserId)) {
            return;
        }
        setMessage("Restoring purchases…");
        Purchases.getSharedInstance().restorePurchases(new ReceiveCustomerInfoCallback() {
            @Override
            public void onReceived(CustomerInfo customerInfo) {
                returnToGame("restored", null);
            }

            @Override
            public void onError(PurchasesError error) {
                returnToGame("error", null);
            }
        });
    }

    private boolean configure(String appUserId) {
        String key = BuildConfig.REVENUECAT_ANDROID_PUBLIC_KEY.trim();
        if (key.isEmpty()) {
            fail("RevenueCat is not configured");
            return false;
        }
        try {
            if (Purchases.isConfigured()) {
                String configuredUser = Purchases.getSharedInstance().getAppUserID();
                if (!appUserId.equals(configuredUser)) {
                    Log.e(TAG, "purchase requested for a different player account");
                    fail("Restart the game before changing accounts");
                    return false;
                }
                return true;
            }
            Purchases.configure(new PurchasesConfiguration.Builder(this, key)
                    .appUserID(appUserId)
                    .build());
            return true;
        } catch (RuntimeException error) {
            Log.e(TAG, "RevenueCat configuration failed", error);
            fail("Store configuration failed");
            return false;
        }
    }

    private void setMessage(String message) {
        TextView view = new TextView(this);
        view.setText(message);
        view.setTextSize(18);
        view.setPadding(48, 48, 48, 48);
        setContentView(view);
    }

    private void fail(String message) {
        setMessage(message);
        returnToGame("error", null);
    }

    private void returnToGame(String status, String productId) {
        Uri.Builder uri = Uri.parse(RETURN_URL).buildUpon()
                .appendQueryParameter("purchase", status);
        if (productId != null) {
            uri.appendQueryParameter("product_id", productId);
        }
        startActivity(new Intent(Intent.ACTION_VIEW, uri.build())
                .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP));
        finish();
    }

    private static boolean isPurchaseUserId(String value) {
        if (value == null || !value.matches("p_[0-9a-f]{24}")) {
            return false;
        }
        return true;
    }
}
