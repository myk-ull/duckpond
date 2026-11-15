// Duck Pond Settings: a native macOS preferences window.
//
// Reads <config>/catalog/{catalog.json, items.json, ducks.png, hats.png}
// (written by the pond) and edits <config>/preferences.json. The pond watches
// that file and applies every change live.
//
// Build: scripts/build_prefs.sh  (swiftc, macOS 13+)

import AppKit
import SwiftUI

// MARK: - Paths

enum Paths {
    static let config: URL = {
        let args = CommandLine.arguments
        if let i = args.firstIndex(of: "--config"), i + 1 < args.count {
            return URL(fileURLWithPath: args[i + 1], isDirectory: true)
        }
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return base.appendingPathComponent("DuckPond", isDirectory: true)
    }()
    static var prefs: URL { config.appendingPathComponent("preferences.json") }
    static var catalog: URL { config.appendingPathComponent("catalog", isDirectory: true) }

    static var requestedFile: String? {
        let args = CommandLine.arguments
        if let i = args.firstIndex(of: "--file"), i + 1 < args.count { return args[i + 1] }
        return nil
    }
}

// MARK: - Model (mirrors src/items.rs `Prefs`)

struct FilePref: Codable, Equatable {
    var breed: String?
    var age: String?
    var hat: String?
    var isEmpty: Bool { breed == nil && age == nil && hat == nil }
}

struct Prefs: Codable, Equatable {
    var version = 3
    var open_at_login = true
    var muted = false
    var volume = 0.6
    var show_labels = false
    var show_hats = false
    var day_night = true
    var mix = ["pekin", "buttercup"]
    var kind_breeds: [String: String] = ["folder": "pekin", "app": "pekin", "pdf": "buttercup", "doc": "buttercup", "code": "buttercup"]
    var kind_hats: [String: String] = [
        "folder": "straw", "image": "beret", "video": "cap", "audio": "headphones", "pdf": "mortar",
        "doc": "glasses", "code": "propeller", "archive": "party", "app": "crown", "alias": "bow", "other": "flower",
    ]
    var duckling_share = 0.8
    var files: [String: FilePref] = [:]

    init() {}

    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        let def = Prefs()
        version = try c.decodeIfPresent(Int.self, forKey: .version) ?? def.version
        open_at_login = try c.decodeIfPresent(Bool.self, forKey: .open_at_login) ?? def.open_at_login
        muted = try c.decodeIfPresent(Bool.self, forKey: .muted) ?? def.muted
        volume = try c.decodeIfPresent(Double.self, forKey: .volume) ?? def.volume
        show_labels = try c.decodeIfPresent(Bool.self, forKey: .show_labels) ?? def.show_labels
        show_hats = try c.decodeIfPresent(Bool.self, forKey: .show_hats) ?? def.show_hats
        day_night = try c.decodeIfPresent(Bool.self, forKey: .day_night) ?? def.day_night
        mix = try c.decodeIfPresent([String].self, forKey: .mix) ?? def.mix
        kind_breeds = try c.decodeIfPresent([String: String].self, forKey: .kind_breeds) ?? def.kind_breeds
        kind_hats = try c.decodeIfPresent([String: String].self, forKey: .kind_hats) ?? def.kind_hats
        duckling_share = try c.decodeIfPresent(Double.self, forKey: .duckling_share) ?? def.duckling_share
        files = try c.decodeIfPresent([String: FilePref].self, forKey: .files) ?? def.files
    }
}

struct Catalog: Decodable {
    struct Breed: Decodable, Identifiable { let id: String; let name: String; let row: Int; let baby_row: Int }
    struct Hat: Decodable, Identifiable { let id: String; let name: String; let x: Int; let w: Int; let h: Int }
    struct Kind: Decodable, Identifiable { let id: String; let name: String }
    let cell: [Int]
    let breeds: [Breed]
    let hats: [Hat]
    let kinds: [Kind]
}

struct FileItem: Decodable, Identifiable, Equatable {
    let key: String
    let name: String
    let kind: String
    let breed: String
    let baby: Bool
    let hat: String?
    var id: String { key }
}

// MARK: - Sprites

final class Sprites {
    private let ducks: CGImage?
    private let hats: CGImage?
    private let cellW: Int
    private let cellH: Int
    private var cache: [String: NSImage] = [:]

    init(cell: [Int]) {
        func load(_ name: String) -> CGImage? {
            guard let img = NSImage(contentsOf: Paths.catalog.appendingPathComponent(name)) else { return nil }
            var r = CGRect(origin: .zero, size: img.size)
            return img.cgImage(forProposedRect: &r, context: nil, hints: nil)
        }
        ducks = load("ducks.png")
        hats = load("hats.png")
        cellW = cell.first ?? 20
        cellH = cell.count > 1 ? cell[1] : 18
    }

    /// One duck cell (column 0 = standing idle, facing right).
    func duck(row: Int, col: Int = 0) -> NSImage? {
        let key = "d\(row):\(col)"
        if let c = cache[key] { return c }
        guard let ducks, let cg = ducks.cropping(to: CGRect(x: col * cellW, y: row * cellH, width: cellW, height: cellH)) else { return nil }
        let img = NSImage(cgImage: cg, size: NSSize(width: cellW, height: cellH))
        cache[key] = img
        return img
    }

    func hat(_ h: Catalog.Hat) -> NSImage? {
        let key = "h\(h.id)"
        if let c = cache[key] { return c }
        guard let hats, let cg = hats.cropping(to: CGRect(x: h.x, y: 0, width: h.w, height: h.h)) else { return nil }
        let img = NSImage(cgImage: cg, size: NSSize(width: h.w, height: h.h))
        cache[key] = img
        return img
    }
}

/// A crisp, pixel-perfect sprite at an integer scale.
struct PixelSprite: View {
    let image: NSImage?
    var scale: CGFloat = 2
    var body: some View {
        if let image {
            Image(nsImage: image)
                .interpolation(.none)
                .resizable()
                .frame(width: image.size.width * scale, height: image.size.height * scale)
        } else {
            Color.clear.frame(width: 20 * scale, height: 18 * scale)
        }
    }
}

// MARK: - Store

final class Store: ObservableObject {
    @Published var prefs = Prefs() { didSet { if prefs != oldValue && !loading { scheduleSave() } } }
    @Published var catalog: Catalog?
    @Published var items: [FileItem] = []
    @Published var pane: Pane = .general
    @Published var search = ""
    @Published var focusFile: String?
    let sprites: Sprites

    private var loading = false
    private var saveWork: DispatchWorkItem?
    private var lastWritten: Date?
    private var timer: Timer?

    init() {
        let cat = (try? Data(contentsOf: Paths.catalog.appendingPathComponent("catalog.json")))
            .flatMap { try? JSONDecoder().decode(Catalog.self, from: $0) }
        catalog = cat
        sprites = Sprites(cell: cat?.cell ?? [20, 18])
        loadPrefs()
        loadItems()
        if let f = Paths.requestedFile { reveal(file: f) }
        timer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            self?.loadItems()
            self?.reloadIfChangedElsewhere()
        }
    }

    func reveal(file key: String) {
        pane = .files
        focusFile = key
        search = items.first(where: { $0.key == key })?.name ?? URL(fileURLWithPath: key).lastPathComponent
    }

    private func modified(_ url: URL) -> Date? {
        (try? FileManager.default.attributesOfItem(atPath: url.path))?[.modificationDate] as? Date
    }

    func loadPrefs() {
        loading = true
        defer { loading = false }
        if let data = try? Data(contentsOf: Paths.prefs), let p = try? JSONDecoder().decode(Prefs.self, from: data) {
            prefs = p
        }
        lastWritten = modified(Paths.prefs)
    }

    /// The menu-bar toggles also write preferences.json: follow them.
    private func reloadIfChangedElsewhere() {
        guard saveWork == nil, let m = modified(Paths.prefs), m != lastWritten else { return }
        loadPrefs()
    }

    func loadItems() {
        struct Wrap: Decodable { let items: [FileItem] }
        guard let data = try? Data(contentsOf: Paths.catalog.appendingPathComponent("items.json")),
              let w = try? JSONDecoder().decode(Wrap.self, from: data) else { return }
        if w.items != items { items = w.items }
    }

    private func scheduleSave() {
        saveWork?.cancel()
        let work = DispatchWorkItem { [weak self] in self?.save() }
        saveWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2, execute: work)
    }

    /// Merge into the existing JSON so fields this window doesn't know about survive.
    private func save() {
        saveWork = nil
        var merged: [String: Any] = [:]
        if let data = try? Data(contentsOf: Paths.prefs),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] { merged = obj }
        var p = prefs
        p.files = p.files.filter { !$0.value.isEmpty }
        guard let mine = try? JSONEncoder().encode(p),
              let obj = try? JSONSerialization.jsonObject(with: mine) as? [String: Any] else { return }
        for (k, v) in obj { merged[k] = v }
        guard let out = try? JSONSerialization.data(withJSONObject: merged, options: [.prettyPrinted, .sortedKeys]) else { return }
        try? FileManager.default.createDirectory(at: Paths.config, withIntermediateDirectories: true)
        try? out.write(to: Paths.prefs, options: .atomic)
        lastWritten = modified(Paths.prefs)
    }

    // helpers
    func breed(_ id: String) -> Catalog.Breed? { catalog?.breeds.first { $0.id == id } }
    func hat(_ id: String?) -> Catalog.Hat? { catalog?.hats.first { $0.id == id } }
    func kindName(_ id: String) -> String { catalog?.kinds.first { $0.id == id }?.name ?? id.capitalized }

    func filePref(_ key: String) -> Binding<FilePref> {
        Binding(get: { self.prefs.files[key] ?? FilePref() },
                set: { self.prefs.files[key] = $0.isEmpty ? nil : $0 })
    }
}

enum Pane: String, CaseIterable, Identifiable {
    case general, ducks, types, hats, files
    var id: String { rawValue }
    var title: String {
        switch self {
        case .general: return "General"
        case .ducks: return "Ducks & Ducklings"
        case .types: return "File Types"
        case .hats: return "Hats"
        case .files: return "Your Files"
        }
    }
    var symbol: String {
        switch self {
        case .general: return "gearshape"
        case .ducks: return "bird"
        case .types: return "doc.on.doc"
        case .hats: return "crown"
        case .files: return "list.bullet.rectangle"
        }
    }
}

// MARK: - Views

struct RootView: View {
    @EnvironmentObject var store: Store

    var body: some View {
        NavigationSplitView {
            List(Pane.allCases, selection: $store.pane) { p in
                Label(p.title, systemImage: p.symbol).tag(p)
            }
            .navigationSplitViewColumnWidth(min: 190, ideal: 200, max: 240)
        } detail: {
            Group {
                if store.catalog == nil {
                    ContentUnavailable()
                } else {
                    switch store.pane {
                    case .general: GeneralPane()
                    case .ducks: DucksPane()
                    case .types: TypesPane()
                    case .hats: HatsPane()
                    case .files: FilesPane()
                    }
                }
            }
            .navigationTitle(store.pane.title)
        }
        .frame(minWidth: 760, minHeight: 540)
    }
}

struct ContentUnavailable: View {
    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: "bird").font(.system(size: 40)).foregroundStyle(.secondary)
            Text("Duck Pond isn't running").font(.title3)
            Text("Start Duck Pond once so it can tell Settings about your ducks.")
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct GeneralPane: View {
    @EnvironmentObject var store: Store
    @State private var confirmReset = false

    var body: some View {
        Form {
            Section("Pond") {
                Toggle("Open Duck Pond at login", isOn: $store.prefs.open_at_login)
                Toggle(isOn: $store.prefs.day_night) {
                    Text("Day and night")
                    Text("Golden hour in the evening, fireflies after dark.")
                }
                Toggle(isOn: $store.prefs.show_labels) {
                    Text("Always show name tags")
                    Text("Otherwise hover a duck, or hold ⌥ to see every name.")
                }
            }
            Section("Sound") {
                Toggle("Quacks and peeps", isOn: Binding(get: { !store.prefs.muted }, set: { store.prefs.muted = !$0 }))
                HStack {
                    Image(systemName: "speaker.fill").foregroundStyle(.secondary)
                    Slider(value: $store.prefs.volume, in: 0...1)
                    Image(systemName: "speaker.wave.3.fill").foregroundStyle(.secondary)
                }
                .disabled(store.prefs.muted)
            }
            Section {
                HStack {
                    Spacer()
                    Button("Restore Defaults…") { confirmReset = true }
                }
            }
        }
        .formStyle(.grouped)
        .confirmationDialog("Restore all Duck Pond settings to their defaults?", isPresented: $confirmReset) {
            Button("Restore Defaults", role: .destructive) { store.prefs = Prefs() }
        } message: {
            Text("Your per-file choices will be cleared too. Ducks keep their spots on the desktop.")
        }
    }
}

struct DucksPane: View {
    @EnvironmentObject var store: Store
    private let columns = [GridItem(.adaptive(minimum: 150), spacing: 12)]

    private var shareText: String {
        let s = store.prefs.duckling_share
        if s < 0.01 { return "No ducklings: everyone's grown up" }
        let perAdult = s / max(0.05, 1 - s)
        let ratio = perAdult >= 1 ? String(format: "about %.0f duckling%@ for every grown-up", perAdult.rounded(), perAdult.rounded() == 1 ? "" : "s")
                                  : "about one duckling for every \(Int((1 / perAdult).rounded())) grown-ups"
        return "\(Int((s * 100).rounded()))% ducklings: \(ratio)"
    }

    var body: some View {
        Form {
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(alignment: .bottom, spacing: 18) {
                        PixelSprite(image: store.sprites.duck(row: 0), scale: 3)
                        HStack(alignment: .bottom, spacing: 2) {
                            ForEach(0..<min(5, max(0, Int((store.prefs.duckling_share / max(0.05, 1 - store.prefs.duckling_share)).rounded()))), id: \.self) { _ in
                                PixelSprite(image: store.sprites.duck(row: 1), scale: 2)
                            }
                        }
                        Spacer()
                    }
                    Slider(value: $store.prefs.duckling_share, in: 0...0.95, step: 0.05) {
                        Text("Ducklings")
                    } minimumValueLabel: {
                        Text("Fewer")
                    } maximumValueLabel: {
                        Text("More")
                    }
                    Text(shareText).font(.callout).foregroundStyle(.secondary)
                }
                .padding(.vertical, 4)
            } header: {
                Text("Ducklings")
            } footer: {
                Text("Folders and apps grow up first, then your biggest files. Ducklings follow a grown-up around the pond.")
            }

            Section {
                LazyVGrid(columns: columns, spacing: 12) {
                    ForEach(store.catalog?.breeds ?? []) { b in
                        BreedCard(breed: b)
                    }
                }
                .padding(.vertical, 6)
            } header: {
                Text("Ducks in the mix")
            } footer: {
                Text("Files set to “Mix” hatch as one of these, chosen by name, so each file always keeps the same duck.")
            }
        }
        .formStyle(.grouped)
    }
}

struct BreedCard: View {
    @EnvironmentObject var store: Store
    let breed: Catalog.Breed

    private var on: Bool { store.prefs.mix.contains(breed.id) }

    var body: some View {
        Button {
            if on {
                if store.prefs.mix.count > 1 { store.prefs.mix.removeAll { $0 == breed.id } }
            } else {
                store.prefs.mix.append(breed.id)
            }
        } label: {
            VStack(spacing: 6) {
                HStack(alignment: .bottom, spacing: 4) {
                    PixelSprite(image: store.sprites.duck(row: breed.row), scale: 3)
                    PixelSprite(image: store.sprites.duck(row: breed.baby_row), scale: 2)
                }
                .frame(height: 56)
                HStack(spacing: 5) {
                    Image(systemName: on ? "checkmark.circle.fill" : "circle")
                        .foregroundStyle(on ? Color.accentColor : Color.secondary)
                    Text(breed.name).lineLimit(1)
                }
                .font(.callout)
            }
            .frame(maxWidth: .infinity)
            .padding(10)
            .background(
                RoundedRectangle(cornerRadius: 10)
                    .fill(on ? Color.accentColor.opacity(0.12) : Color.secondary.opacity(0.06))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10)
                    .strokeBorder(on ? Color.accentColor.opacity(0.6) : Color.secondary.opacity(0.15))
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(on && store.prefs.mix.count == 1 ? "Keep at least one duck in the mix" : "")
    }
}

/// Picker of breeds with sprite icons. `extra` adds e.g. ("mix", "Mix").
struct BreedPicker: View {
    @EnvironmentObject var store: Store
    let title: String
    @Binding var selection: String
    var extra: [(String, String)]

    var body: some View {
        Picker(title, selection: $selection) {
            ForEach(extra, id: \.0) { e in Text(e.1).tag(e.0) }
            Divider()
            ForEach(store.catalog?.breeds ?? []) { b in
                Label {
                    Text(b.name)
                } icon: {
                    PixelSprite(image: store.sprites.duck(row: b.row), scale: 1)
                }
                .tag(b.id)
            }
        }
    }
}

struct HatPicker: View {
    @EnvironmentObject var store: Store
    let title: String
    @Binding var selection: String
    var extra: [(String, String)]

    var body: some View {
        Picker(title, selection: $selection) {
            ForEach(extra, id: \.0) { e in Text(e.1).tag(e.0) }
            Divider()
            ForEach(store.catalog?.hats ?? []) { h in
                Label {
                    Text(h.name)
                } icon: {
                    PixelSprite(image: store.sprites.hat(h), scale: 2)
                }
                .tag(h.id)
            }
        }
    }
}

struct TypesPane: View {
    @EnvironmentObject var store: Store

    private func binding(_ kind: String) -> Binding<String> {
        Binding(get: { store.prefs.kind_breeds[kind] ?? "mix" },
                set: { store.prefs.kind_breeds[kind] = $0 })
    }

    var body: some View {
        Form {
            Section {
                ForEach(store.catalog?.kinds ?? []) { k in
                    HStack {
                        let current = store.prefs.kind_breeds[k.id] ?? "mix"
                        PixelSprite(image: store.breed(current).flatMap { store.sprites.duck(row: $0.row) }, scale: 1.5)
                            .opacity(current == "mix" ? 0 : 1)
                            .frame(width: 32)
                        BreedPicker(title: k.name, selection: binding(k.id), extra: [("mix", "Mix")])
                    }
                }
            } header: {
                Text("Which duck each kind of file hatches as")
            } footer: {
                Text("“Mix” picks from the ducks ticked under Ducks & Ducklings. Individual files can be changed under Your Files.")
            }
        }
        .formStyle(.grouped)
    }
}

struct HatsPane: View {
    @EnvironmentObject var store: Store

    private func binding(_ kind: String) -> Binding<String> {
        Binding(get: { store.prefs.kind_hats[kind] ?? "none" },
                set: { store.prefs.kind_hats[kind] = $0 })
    }

    var body: some View {
        Form {
            Section {
                Toggle(isOn: $store.prefs.show_hats) {
                    Text("Ducks wear hats")
                    Text("Each kind of file gets its own hat, so you can tell them apart at a glance.")
                }
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 14) {
                        ForEach(store.catalog?.hats ?? []) { h in
                            VStack(spacing: 4) {
                                PixelSprite(image: store.sprites.hat(h), scale: 3).frame(height: 30)
                                Text(h.name).font(.caption2).foregroundStyle(.secondary)
                            }
                        }
                    }
                    .padding(.vertical, 4)
                }
            }
            Section {
                ForEach(store.catalog?.kinds ?? []) { k in
                    HatPicker(title: k.name, selection: binding(k.id), extra: [("none", "No hat")])
                }
            } header: {
                Text("Hat for each kind of file")
            } footer: {
                Text("Ducklings swap the big hats for a bow. Individual files can be changed under Your Files.")
            }
            .disabled(!store.prefs.show_hats)
        }
        .formStyle(.grouped)
    }
}

struct FilesPane: View {
    @EnvironmentObject var store: Store
    @State private var onlyCustom = false

    private var rows: [FileItem] {
        store.items.filter { item in
            (!onlyCustom || store.prefs.files[item.key] != nil)
                && (store.search.isEmpty || item.name.localizedCaseInsensitiveContains(store.search))
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Picker("", selection: $onlyCustom) {
                    Text("All Files").tag(false)
                    Text("Customised").tag(true)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .frame(width: 220)
                Spacer()
                Text("\(rows.count) of \(store.items.count)").foregroundStyle(.secondary).font(.callout)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            Divider()
            ScrollViewReader { proxy in
                List(rows) { item in
                    FileRow(item: item, focused: store.focusFile == item.key)
                        .id(item.key)
                }
                .onAppear {
                    if let f = store.focusFile { DispatchQueue.main.async { proxy.scrollTo(f, anchor: .center) } }
                }
                .onReceive(store.$focusFile) { f in
                    if let f { withAnimation { proxy.scrollTo(f, anchor: .center) } }
                }
            }
        }
        .searchable(text: $store.search, placement: .toolbar, prompt: "Search your files")
    }
}

struct FileRow: View {
    @EnvironmentObject var store: Store
    let item: FileItem
    let focused: Bool

    var body: some View {
        let pref = store.filePref(item.key)
        HStack(spacing: 12) {
            ZStack(alignment: .bottomTrailing) {
                if let b = store.breed(item.breed) {
                    PixelSprite(image: store.sprites.duck(row: item.baby ? b.baby_row : b.row), scale: 2)
                }
                if store.prefs.show_hats, let h = store.hat(item.hat) {
                    PixelSprite(image: store.sprites.hat(h), scale: 1.5).offset(x: 4, y: 2)
                }
            }
            .frame(width: 44, height: 38)
            VStack(alignment: .leading, spacing: 2) {
                Text(item.name).lineLimit(1).truncationMode(.middle)
                Text(store.kindName(item.kind) + (item.baby ? " · duckling" : " · grown-up"))
                    .font(.caption).foregroundStyle(.secondary)
            }
            Spacer(minLength: 8)
            BreedPicker(title: "Duck", selection: Binding(
                get: { pref.wrappedValue.breed ?? "auto" },
                set: { pref.wrappedValue.breed = $0 == "auto" ? nil : $0 }), extra: [("auto", "Automatic")])
                .labelsHidden()
                .frame(width: 150)
            Picker("Age", selection: Binding(
                get: { pref.wrappedValue.age ?? "auto" },
                set: { pref.wrappedValue.age = $0 == "auto" ? nil : $0 })) {
                Text("Automatic").tag("auto")
                Divider()
                Text("Duckling").tag("duckling")
                Text("Grown-up").tag("adult")
            }
            .labelsHidden()
            .frame(width: 110)
            HatPicker(title: "Hat", selection: Binding(
                get: { pref.wrappedValue.hat ?? "auto" },
                set: { pref.wrappedValue.hat = $0 == "auto" ? nil : $0 }), extra: [("auto", "Automatic"), ("none", "No hat")])
                .labelsHidden()
                .frame(width: 150)
                .disabled(!store.prefs.show_hats)
            Button {
                store.prefs.files[item.key] = nil
            } label: {
                Image(systemName: "arrow.uturn.backward.circle")
            }
            .buttonStyle(.borderless)
            .help("Back to automatic")
            .opacity(store.prefs.files[item.key] == nil ? 0 : 1)
            .disabled(store.prefs.files[item.key] == nil)
        }
        .padding(.vertical, 3)
        .listRowBackground(focused ? Color.accentColor.opacity(0.15) : nil)
    }
}

// MARK: - App

final class AppDelegate: NSObject, NSApplicationDelegate {
    static let selectNote = Notification.Name("app.duckpond.settings.select")

    func applicationWillFinishLaunching(_ notification: Notification) {
        // one Settings window at a time: hand our --file to the open one and quit
        let me = ProcessInfo.processInfo.processIdentifier
        let exe = Bundle.main.executableURL?.resolvingSymlinksInPath()
        if let other = NSWorkspace.shared.runningApplications.first(where: {
            $0.processIdentifier != me && $0.executableURL?.resolvingSymlinksInPath() == exe
        }) {
            DistributedNotificationCenter.default().postNotificationName(
                Self.selectNote, object: Paths.requestedFile, userInfo: nil, deliverImmediately: true)
            other.activate()
            exit(0)
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        if let duck = Sprites(cell: [20, 18]).duck(row: 0) {
            // a big pixel duck for the Dock
            let big = NSImage(size: NSSize(width: 512, height: 512), flipped: false) { rect in
                NSGraphicsContext.current?.imageInterpolation = .none
                duck.draw(in: rect.insetBy(dx: 26, dy: 56))
                return true
            }
            NSApp.applicationIconImage = big
        }
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

@main
struct DuckPondSettingsApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var delegate
    @StateObject private var store = Store()

    var body: some Scene {
        Window("Duck Pond Settings", id: "settings") {
            RootView()
                .environmentObject(store)
                .onReceive(DistributedNotificationCenter.default().publisher(for: AppDelegate.selectNote)) { note in
                    if let key = note.object as? String { store.reveal(file: key) }
                    NSApp.activate(ignoringOtherApps: true)
                }
        }
        .windowResizability(.contentMinSize)
        .commands { CommandGroup(replacing: .newItem) {} }
    }
}
