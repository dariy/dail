# R8 keep rules for the release build (minified and shrunk).

# JNI: Rust binds the `Native` exports by name (`Java_net_darii_dail_Native_*`) and calls
# `Listener.wake` and `Listener.alert` on whatever object Kotlin handed it.
-keep class net.darii.dail.Native { *; }
-keep interface net.darii.dail.Listener { *; }
-keep class * implements net.darii.dail.Listener { *; }
-keepclasseswithmembernames,includedescriptorclasses class * { native <methods>; }

# kotlinx.serialization: the frame, event and alert models decode by their generated serializers,
# which R8 must neither strip nor rename (field names are the JSON keys, through @SerialName or
# the property name). The library ships most rules; these cover the app's own classes.
-keepattributes *Annotation*, InnerClasses, Signature
-dontnote kotlinx.serialization.**
-keepclassmembers @kotlinx.serialization.Serializable class net.darii.dail.** {
    *** Companion;
    kotlinx.serialization.KSerializer serializer(...);
    <fields>;
}
-keepclasseswithmembers class net.darii.dail.** {
    kotlinx.serialization.KSerializer serializer(...);
}
-keep,includedescriptorclasses class net.darii.dail.**$$serializer { *; }
