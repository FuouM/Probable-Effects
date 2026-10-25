// Builds the smoke-test projects for scripts/ae-smoke.ps1. Run by AfterFX.exe -r; inputs come
// from the environment because -r takes no arguments:
//   PROBABLE_SMOKE_DIR    output folder (projects, renders, cases.txt, ae-smoke.log)
//   PROBABLE_SMOKE_IMAGE  source still
// cases.txt has one line per case: name<TAB>match name<TAB>Label=value;Label=value
// One project per bit depth (bit depth is project-wide), one comp per case in each, all queued
// through the built-in "TIFF Sequence with Alpha" template (output module Format is read-only to
// scripts; this is the one lossless sequence template every install has). It writes 8 bpc.
(function () {
    var dir = $.getenv("PROBABLE_SMOKE_DIR");
    var image = $.getenv("PROBABLE_SMOKE_IMAGE");
    var log = [];

    function readCases() {
        var f = new File(dir + "/cases.txt");
        if (!f.open("r")) { throw new Error("cannot read " + f.fsName); }
        var cases = [];
        while (!f.eof) {
            var line = f.readln();
            if (!line) { continue; }
            var cols = line.split("\t");
            cases.push({ name: cols[0], matchName: cols[1], params: cols[2] || "" });
        }
        f.close();
        return cases;
    }

    app.beginSuppressDialogs();
    try {
        var cases = readCases();
        var depths = [8, 16, 32];
        for (var d = 0; d < depths.length; d++) {
            var bpc = depths[d];
            app.newProject();
            app.project.bitsPerChannel = bpc;
            var item = app.project.importFile(new ImportOptions(new File(image)));
            for (var c = 0; c < cases.length; c++) {
                var cs = cases[c];
                var comp = app.project.items.addComp(cs.name, item.width, item.height, 1, 1 / 30, 30);
                var fx = comp.layers.add(item).property("ADBE Effect Parade").addProperty(cs.matchName);
                if (!fx) { throw new Error("effect not found: " + cs.matchName); }
                var pairs = cs.params.split(";");
                for (var i = 0; i < pairs.length; i++) {
                    if (!pairs[i]) { continue; }
                    var kv = pairs[i].split("=");
                    fx.property(kv[0]).setValue(parseFloat(kv[1]));
                }
                var om = app.project.renderQueue.items.add(comp).outputModule(1);
                om.applyTemplate("TIFF Sequence with Alpha");
                om.file = new File(dir + "/ae_" + bpc + "_" + cs.name + "_[#####].tif");
            }
            app.project.save(new File(dir + "/smoke_" + bpc + ".aep"));
            log.push("ok " + bpc + " bpc, " + cases.length + " cases");
        }
    } catch (e) {
        log.push("error: " + e.toString() + " line " + e.line);
    }
    app.endSuppressDialogs(false);
    // Needs "Allow Scripts to Write Files and Access Network" (AE Preferences > Scripting).
    var f = new File(dir + "/ae-smoke.log");
    f.open("w");
    f.write(log.join("\n"));
    f.close();
    // An unsaved project would block app.quit() on a "Save changes?" dialog.
    app.project.close(CloseOptions.DO_NOT_SAVE_CHANGES);
    app.quit();
})();
