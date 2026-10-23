import com.android.build.api.variant.ApplicationVariant

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
    alias(libs.plugins.licensee)
}

/** The ABIs `libdail.so` is built for; `-Pdail.abis=x86_64` narrows a local build. */
val dailAbis: List<String> =
    (findProperty("dail.abis") as String?)?.split(',')?.map(String::trim) ?: listOf("arm64-v8a", "x86_64")

/** The workspace version (`[workspace.package] version`), the app's version name. */
val workspaceVersion: String =
    rootDir
        .resolve("../Cargo.toml")
        .readLines()
        .first { it.startsWith("version = ") }
        .substringAfter('"')
        .substringBefore('"')

/**
 * The release signing key, from the environment only (CI decodes it from repository secrets; it is
 * never in the repository): `DAIL_SIGNING_STORE` (a keystore file), `DAIL_SIGNING_STORE_PASSWORD`,
 * `DAIL_SIGNING_KEY_ALIAS` and `DAIL_SIGNING_KEY_PASSWORD`. Without them the release APKs are
 * unsigned, which is enough for `apk-check`; `scripts/release-android.sh` requires them.
 */
val signingEnv: Map<String, String>? =
    listOf("STORE", "STORE_PASSWORD", "KEY_ALIAS", "KEY_PASSWORD")
        .associateWith { System.getenv("DAIL_SIGNING_$it").orEmpty() }
        .takeIf { env -> env.values.all(String::isNotEmpty) }

android {
    namespace = "net.darii.dail"
    compileSdk = 36
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "net.darii.dail"
        minSdk = 29
        targetSdk = 36
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        versionName = workspaceVersion
        versionCode = workspaceVersion.split('.').fold(0) { code, part -> code * 100 + part.toInt() }
    }

    flavorDimensions += "backend"
    productFlavors {
        create("live") {
            dimension = "backend"
            buildConfigField("String", "BACKEND", "\"\\\"live\\\"\"")
        }
        create("demo") {
            dimension = "backend"
            applicationIdSuffix = ".demo"
            buildConfigField("String", "BACKEND", "\"{\\\"memory\\\": \\\"basic\\\"}\"")
        }
    }

    signingConfigs {
        signingEnv?.let { env ->
            create("release") {
                storeFile = file(env.getValue("STORE"))
                storePassword = env.getValue("STORE_PASSWORD")
                keyAlias = env.getValue("KEY_ALIAS")
                keyPassword = env.getValue("KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.findByName("release")
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }

    splits {
        abi {
            isEnable = true
            reset()
            include(*dailAbis.toTypedArray())
            isUniversalApk = false
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // Android Lint with warnings as errors; `lint.xml` turns off, with reasons, what the pins make moot.
    lint {
        warningsAsErrors = true
        abortOnError = true
        checkDependencies = true
        lintConfig = file("lint.xml")
    }

    testOptions {
        unitTests.isReturnDefaultValues = true
    }
}

kotlin {
    jvmToolchain(17)
}

/**
 * Builds `libdail.so` for [abis] with cargo-ndk (NDK r28+: 16 KB pages, set explicitly too) into
 * `build/rustJniLibs/<variant>`, and nothing else cargo-ndk copies; the variant's jniLibs take it
 * from there.
 */
abstract class CargoNdk : DefaultTask() {
    @get:Input abstract val abis: ListProperty<String>

    @get:Input abstract val release: Property<Boolean>

    @get:Input abstract val demo: Property<Boolean>

    @get:Internal abstract val workspace: DirectoryProperty

    @get:Internal abstract val ndk: DirectoryProperty

    @get:OutputDirectory abstract val output: DirectoryProperty

    @get:Inject abstract val exec: ExecOperations

    @get:Inject abstract val files: FileSystemOperations

    @TaskAction
    fun build() {
        val args = mutableListOf("ndk")
        abis.get().forEach { args += listOf("-t", it) }
        val built = temporaryDir.resolve("jniLibs")
        args += listOf("--platform", "29", "-o", built.path, "build", "-p", "app-android")
        if (release.get()) args += "--release"
        if (demo.get()) args += listOf("--features", "demo")
        val pages = "-C link-arg=-Wl,-z,max-page-size=16384"
        exec.exec {
            workingDir = workspace.get().asFile
            commandLine(listOf("cargo") + args)
            environment("ANDROID_NDK_HOME", ndk.get().asFile.path)
            environment("CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS", pages)
            environment("CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS", pages)
        }
        // cargo-ndk copies every cdylib it built, a dependency's too; the APK carries only ours.
        files.sync {
            from(built)
            into(output)
            include("*/libdail.so")
        }
    }
}

/** `app-theme`'s fonts as Android font resources (lower-case names), copied, never duplicated. */
abstract class ThemeFonts : DefaultTask() {
    @get:InputDirectory abstract val fonts: DirectoryProperty

    @get:OutputDirectory abstract val output: DirectoryProperty

    @get:Inject abstract val files: FileSystemOperations

    @TaskAction
    fun copy() {
        val names =
            mapOf(
                "Geist[wght].ttf" to "geist.ttf",
                "JetBrainsMono[wght].ttf" to "jetbrains_mono.ttf",
                "MaterialSymbolsOutlined-dail.ttf" to "material_symbols.ttf",
            )
        files.sync {
            from(fonts)
            into(output.dir("font"))
            include(names.keys)
            rename { names.getValue(it) }
        }
    }
}

val themeFonts =
    tasks.register<ThemeFonts>("themeFonts") {
        fonts.set(rootProject.layout.projectDirectory.dir("../crates/app-theme/assets/fonts"))
        output.set(layout.buildDirectory.dir("themeFonts"))
    }

androidComponents {
    onVariants { variant: ApplicationVariant ->
        variant.sources.res?.addGeneratedSourceDirectory(themeFonts, ThemeFonts::output)
        val name = variant.name.replaceFirstChar(Char::uppercase)
        val cargo =
            tasks.register<CargoNdk>("cargoNdk$name") {
                abis.set(dailAbis)
                release.set(variant.buildType == "release")
                demo.set(variant.flavorName == "demo")
                workspace.set(rootProject.layout.projectDirectory.dir(".."))
                ndk.set(androidComponents.sdkComponents.ndkDirectory)
                output.set(layout.buildDirectory.dir("rustJniLibs/${variant.name}"))
                // Cargo decides what is stale; Gradle always asks it.
                outputs.upToDateWhen { false }
            }
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargo, CargoNdk::output)
    }
}

// The licence allowlist (the Gradle side of `deny.toml`'s `[licenses]`): an artifact under any other
// licence fails `licensee`, which `check` runs.
licensee {
    allow("Apache-2.0")
    allow("MIT")
    allow("BSD-2-Clause")
    allow("BSD-3-Clause")
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    implementation(libs.kotlinx.serialization.json)

    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.junit)
}
