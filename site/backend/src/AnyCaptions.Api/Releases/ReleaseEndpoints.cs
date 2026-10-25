using System.Security.Claims;
using AnyCaptions.Api.Data;
using AnyCaptions.Api.Domain.Entities;
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
                    item.Version, item.ReleaseNotes, item.WindowsInstallerUrl != "",
                    item.WindowsZipUrl != null, item.Sha256, item.PublishedAt))
                .FirstOrDefaultAsync(ct);
            return release is null ? Results.NotFound() : Results.Ok(release);
        });
        group.MapGet("/", async (AppDbContext db, CancellationToken ct) =>
            Results.Ok(await db.Releases.AsNoTracking().Where(item => item.IsActive)
                .OrderByDescending(item => item.PublishedAt).Select(item => new ReleaseResponse(
                    item.Version, item.ReleaseNotes, item.WindowsInstallerUrl != "",
                    item.WindowsZipUrl != null, item.Sha256, item.PublishedAt))
                .ToListAsync(ct)));
        // Downloads require a signed-in account; each one is recorded per user and release.
        group.MapGet("/latest/download", async (string? kind, ClaimsPrincipal principal,
            AppDbContext db, CancellationToken ct) =>
        {
            if (!Guid.TryParse(principal.FindFirstValue(ClaimTypes.NameIdentifier), out var userId))
                return Results.Unauthorized();
            var requested = kind is null or "installer" ? "installer" : kind == "zip" ? "zip" : null;
            if (requested is null) return Results.BadRequest();
            var release = await db.Releases.Where(item => item.IsActive)
                .OrderByDescending(item => item.PublishedAt).FirstOrDefaultAsync(ct);
            var url = requested == "zip" ? release?.WindowsZipUrl : release?.WindowsInstallerUrl;
            if (release is null || string.IsNullOrEmpty(url)) return Results.NotFound();
            db.Downloads.Add(new Download { UserId = userId, ReleaseId = release.Id, Kind = requested });
            await db.SaveChangesAsync(ct);
            return Results.Redirect(url);
        }).RequireAuthorization();
    }
}

public sealed record ReleaseResponse(string Version, string ReleaseNotes,
    bool HasInstaller, bool HasZip, string Sha256, DateTimeOffset PublishedAt);
