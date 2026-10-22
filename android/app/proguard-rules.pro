# JNI: Rust calls these by name.
-keep class net.darii.dail.Native { *; }
-keep interface net.darii.dail.Listener { *; }
-keep class * implements net.darii.dail.Listener { *; }
