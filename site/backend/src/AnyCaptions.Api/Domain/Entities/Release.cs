namespace AnyCaptions.Api.Domain.Entities;

public sealed class Release
{
    public Guid Id { get; set; } = Guid.NewGuid();
    public required string Version { get; set; }
    public required string ReleaseNotes { get; set; }
    public required string WindowsInstallerUrl { get; set; }
    public string? WindowsZipUrl { get; set; }
    public required string Sha256 { get; set; }
    public DateTimeOffset PublishedAt { get; set; }
    public bool IsActive { get; set; }
}
