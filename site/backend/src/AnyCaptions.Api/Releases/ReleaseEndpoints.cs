using AnyCaptions.Api.Data;
using Microsoft.EntityFrameworkCore;

namespace AnyCaptions.Api.Releases;

public static class ReleaseEndpoints
{
    public static void MapReleaseEndpoints(this WebApplication app)
    {
        var group = app.MapGroup("/api/releases");
        group.MapGet("/latest", async (AppDbContext db, CancellationToken ct) =>
        {
            var release = await db.Releases.AsNoTracking().Where(item => item.IsActive)
                .OrderByDescending(item => item.PublishedAt).Select(item => new ReleaseResponse(
                    item.Version, item.ReleaseNotes, item.WindowsInstallerUrl,
                    item.WindowsZipUrl, item.Sha256, item.PublishedAt))
                .FirstOrDefaultAsync(ct);
            return release is null ? Results.NotFound() : Results.Ok(release);
        });
        group.MapGet("/", async (AppDbContext db, CancellationToken ct) =>
            Results.Ok(await db.Releases.AsNoTracking().Where(item => item.IsActive)
                .OrderByDescending(item => item.PublishedAt).Select(item => new ReleaseResponse(
                    item.Version, item.ReleaseNotes, item.WindowsInstallerUrl,
                    item.WindowsZipUrl, item.Sha256, item.PublishedAt))
                .ToListAsync(ct)));
    }
}

public sealed record ReleaseResponse(string Version, string ReleaseNotes,
    string WindowsInstallerUrl, string? WindowsZipUrl, string Sha256, DateTimeOffset PublishedAt);
