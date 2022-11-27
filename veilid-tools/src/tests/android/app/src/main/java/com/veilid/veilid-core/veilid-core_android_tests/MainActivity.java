package com.veilid.veilidtools.veilidtools_android_tests;

import androidx.appcompat.app.AppCompatActivity;
import android.content.Context;
import android.os.Bundle;

public class MainActivity extends AppCompatActivity {

    static {
        System.loadLibrary("veilid_tools");
    }

    private static native void run_tests(Context context);

    private Thread testThread;

    class TestThread extends Thread {
        private Context context;

        TestThread(Context context) {
            this.context = context;
        }

        public void run() {
            run_tests(this.context);
        }
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);

        this.testThread = new TestThread(this);
        this.testThread.start();
    }
}
