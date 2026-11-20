// Dear ImGui version of the `list(N)` workload in benchmarks/src/main.rs.
// Builds against an upstream Dear ImGui checkout (MIT); see run_imgui_bench.sh.
// Measures NewFrame → widgets → Render (draw-data generation), no GPU backend.

#include "imgui.h"

#include <algorithm>
#include <chrono>
#include <cstdio>
#include <string>
#include <vector>

static const int kWarmupFrames = 100;
static const int kMeasuredFrames = 1000;

// Prints one result line unless `quiet`. `formatted` formats each label every
// frame; otherwise the labels are pre-formatted strings.
static void bench(int n, bool formatted, bool quiet = false) {
    ImGui::CreateContext();
    ImGuiIO& io = ImGui::GetIO();
    io.DisplaySize = ImVec2(1280, 800);
    io.DisplayFramebufferScale = ImVec2(2, 2);
    io.IniFilename = nullptr;
    unsigned char* px; int w, h;
    io.Fonts->GetTexDataAsRGBA32(&px, &w, &h);  // legacy atlas path: built up front
    io.Fonts->SetTexID((ImTextureID)1);
    std::vector<char> checks(n, 0);
    std::vector<float> values(n);
    std::vector<std::string> labels(n);
    char tmp[64];
    for (int i = 0; i < n; ++i) {
        values[i] = float(i % 100);
        std::snprintf(tmp, sizeof tmp, "Row %d: %.1f", i, values[i]);
        labels[i] = tmp;
    }
    std::vector<double> us;
    int vtx = 0, cmds = 0;
    for (int f = 0; f < kWarmupFrames + kMeasuredFrames; ++f) {
        io.DeltaTime = 1.0f / 60.0f;
        auto t0 = std::chrono::steady_clock::now();
        ImGui::NewFrame();
        ImGui::SetNextWindowPos(ImVec2(0, 0));
        ImGui::SetNextWindowSize(io.DisplaySize);
        ImGui::Begin("main", nullptr, ImGuiWindowFlags_NoDecoration);
        ImGui::BeginChild("list");
        for (int i = 0; i < n; ++i) {
            ImGui::PushID(i);
            if (formatted) ImGui::Text("Row %d: %.1f", i, values[i]);
            else ImGui::TextUnformatted(labels[i].c_str());
            ImGui::SameLine();
            ImGui::Button("Action");
            ImGui::SameLine();
            bool c = checks[i] != 0;
            ImGui::Checkbox("On", &c);
            checks[i] = c;
            ImGui::SameLine();
            ImGui::SetNextItemWidth(200);
            ImGui::SliderFloat("Value", &values[i], 0.0f, 100.0f);
            ImGui::PopID();
        }
        ImGui::EndChild();
        ImGui::End();
        ImGui::Render();
        auto t1 = std::chrono::steady_clock::now();
        if (f >= kWarmupFrames) {
            us.push_back(std::chrono::duration<double, std::micro>(t1 - t0).count());
            ImDrawData* dd = ImGui::GetDrawData();
            vtx = dd->TotalVtxCount;
            cmds = 0;
            for (ImDrawList* l : dd->CmdLists) cmds += l->CmdBuffer.Size;
        }
    }
    std::sort(us.begin(), us.end());
    double mean = 0;
    for (double v : us) mean += v;
    mean /= us.size();
    if (!quiet) std::printf("imgui  %s(%4d) CPU NewFrame..Render: mean %7.1f us  p50 %7.1f  p99 %7.1f | %6d vertices %3d draw cmds\n", formatted ? "fmt   " : "static", n,
                mean, us[us.size() / 2], us[(size_t)((us.size() - 1) * 0.99)], vtx, cmds);
    ImGui::DestroyContext();
}

int main() {
    std::printf("Dear ImGui %s\n", IMGUI_VERSION);
    // About a second of unmeasured work so the CPU clock has ramped up, as in the Rust benchmark.
    auto t0 = std::chrono::steady_clock::now();
    while (std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count() < 1.0) bench(200, true, true);
    for (int n : {20, 50, 200, 1000}) bench(n, false);
    for (int n : {50, 200, 1000}) bench(n, true);
}
