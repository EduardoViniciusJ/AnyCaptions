namespace AnyCaptions.Api.Domain.Entities;

public sealed class Download
{
    public Guid Id { get; set; } = Guid.NewGuid();
    public Guid UserId { get; set; }
    public User? User { get; set; }
    public Guid ReleaseId { get; set; }
    public Release? Release { get; set; }
    public required string Kind { get; set; }
    public DateTimeOffset CreatedAt { get; set; } = DateTimeOffset.UtcNow;
}
