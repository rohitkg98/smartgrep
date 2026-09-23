// Test fixture for Java parser tests.
// Contains a variety of Java constructs.

package com.example.demo;

import java.util.List;
import java.io.Serializable;

public class Config implements Serializable {
    private String name;
    public List<String> values;
    int timeout;

    public Config(String name) {
        this.name = name;
        this.timeout = 30;
    }

    public String getName() {
        return name;
    }

    public void addValue(String v) {
        values.add(v);
    }

    private static int maxSize() {
        return 1024;
    }
}

public interface Processor<T> {
    void process(T input);
    String name();
}

public enum Status {
    ACTIVE,
    INACTIVE,
    ERROR;

    public String label() {
        return name().toLowerCase();
    }
}

public record Point(int x, int y) {
    public double distance() {
        return Math.sqrt(x * x + y * y);
    }
}

@Deprecated
class InternalHelper extends Config implements Processor<String> {
    public InternalHelper() {
        super("helper");
    }

    @Override
    public void process(String input) {
        System.out.println(input);
    }

    @Override
    public String name() {
        return "helper";
    }
}

// Sealed interface with inner records
public sealed interface Action {
    record Create(String name) implements Action {}
    record Delete(String id) implements Action {}
}

// Class with various nested types
public class Container {
    public static class Nested {
        public void nestedMethod() {}
    }
    public enum NestedEnum { X, Y }
    public interface NestedInterface {
        void act();
    }
    public record NestedRecord(int val) {}

    // Call-graph fixture: exercises call deps
    public static class Dispatcher {
        private final List<String> items = new ArrayList<>();
        private static final Logger LOG = Logger.getLogger("dispatcher");

        public Dispatcher(String name) {
            super();
            init(name);
        }

        void init(String name) {}

        public void run(List<String> xs) {
            helper();
            helper();
            Collections.sort(xs);
            java.util.Objects.requireNonNull(xs);
            this.items.add("a");
            items.clear();
            LOG.info("run");
            Map<String, List<Integer>> m = new HashMap<String, List<Integer>>();
            xs.forEach(x -> process(x));
            xs.stream().map(String::trim).forEach(this::consume);
            Runnable r = new Runnable() {
                public void run() { anonCall(); }
            };
            super.toString();
            build().finish();
        }
    }
}
